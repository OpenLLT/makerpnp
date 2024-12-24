use tracing::Level;
use termtree::Tree;
use std::path::PathBuf;
use anyhow::Error;
use crux_core::App;
use crux_core::macros::Effect;
use crux_core::render::Render;
use serde_with::serde_as;

pub use crux_core::Core;
use csv::QuoteStyle;
use thiserror::Error;
use tracing::{error, info, trace};
pub use assembly::assembly_variant::AssemblyVariant;
use assembly::AssemblyVariantProcessor;
pub use eda::EdaTool;
use eda::placement::{EdaPlacement, EdaPlacementField};
use eda::substitution::{EdaSubstitutionResult, EdaSubstitutionRule, EdaSubstitutor};
use part_mapper::{PartMapper, PartMapperError, PartMappingError, PartMappingResult, PlacementPartMappingResult};
use stores::{assembly_rules, eda_placements, load_out, part_mappings, parts, substitutions};
pub use stores::load_out::LoadOutSource;
use stores::placements::PlacementRecord;

extern crate serde_regex;

#[derive(Default)]
pub struct VariantBuilder;

#[derive(Default)]
pub struct Model {
    error: Option<String>
}

#[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug)]
pub struct OperationViewModel {
    pub error: Option<String>
}

#[derive(Effect)]
pub struct Capabilities {
    render: Render<Event>,
}

#[serde_as]
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum Event {
    None,
    Build {
        eda_tool: EdaTool,
        placements: String,
        assembly_variant: AssemblyVariant,
        parts: String,
        part_mappings: String,
        substitutions: Vec<String>,
        load_out: Option<LoadOutSource>,
        assembly_rules: Option<String>,
        output: String,
        ref_des_disable_list: Vec<String>,
    },

    //
    // Views
    //
}

impl App for VariantBuilder {
    type Event = Event;
    type Model = Model;
    type ViewModel = OperationViewModel;
    type Capabilities = Capabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        #[allow(unused_mut)]
        let mut default_render = true;
        match event {
            Event::None => {}
            Event::Build {
                eda_tool,
                placements,
                assembly_variant,
                parts,
                part_mappings,
                substitutions,
                load_out,
                assembly_rules,
                output,
                ref_des_disable_list
            } => {
                let try_fn = |_model: &mut Model| -> Result<(), AppError> {

                    let result = build_assembly_variant(
                        eda_tool,
                        &placements,
                        assembly_variant,
                        &parts,
                        &part_mappings,
                        &substitutions,
                        &load_out,
                        &assembly_rules,
                        &output,
                        &ref_des_disable_list
                    )
                        .map_err(|cause|AppError::OperationError(cause.into()))
                        ?;

                    Ok(result)
                };

                if let Err(e) = try_fn(model) {
                    model.error.replace(format!("{:?}", e));
                };
            }
        }

        if default_render {
            // This causes the shell to request the view, via `view()`
            caps.render.render();
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        OperationViewModel {
            error: model.error.clone(),
        }
    }
}

#[derive(Error, Debug)]
enum AppError {
    #[error("Operation error, cause: {0}")]
    OperationError(anyhow::Error),
}


#[tracing::instrument(level = Level::DEBUG)]
fn build_assembly_variant(
    eda_tool: EdaTool,
    placements_source: &String,
    assembly_variant: AssemblyVariant,
    parts_source: &String,
    part_mappings_source: &String,
    eda_substitutions_sources: &[String],
    load_out_source: &Option<LoadOutSource>,
    assembly_rules_source: &Option<String>,
    output: &String,
    ref_des_disable_list: &Vec<String>
) -> Result<(), Error> {

    let mut original_eda_placements = eda_placements::load_eda_placements(eda_tool, placements_source)?;
    info!("Loaded {} placements", original_eda_placements.len());

    let eda_substitution_rules = eda_substitutions_sources.iter().try_fold(vec![], |mut rules, source| {
        let source_rules = substitutions::load_eda_substitutions(source)?;
        info!("Loaded {} substitution rules from {}", source_rules.len(), source);
        rules.extend(source_rules);

        Ok::<Vec<EdaSubstitutionRule>, anyhow::Error>(rules)
    })?;

    let eda_substitution_results = EdaSubstitutor::substitute(original_eda_placements.as_mut_slice(), eda_substitution_rules.as_slice());
    trace!("eda_substitution_results: {:?}", eda_substitution_results);

    info!("disabling placements: {:?}", ref_des_disable_list);
    let mut eda_placements: Vec<EdaPlacement> = eda_substitution_results.iter().map(|esr| esr.resulting_placement.clone()).collect();

    for eda_placement in eda_placements.iter_mut() {
        if ref_des_disable_list.contains(&eda_placement.ref_des) {
            eda_placement.place = false;
        }
    }

    let parts = parts::load_parts(parts_source)?;
    info!("Loaded {} parts", parts.len());

    let part_mappings = part_mappings::load_part_mappings(&parts, part_mappings_source)?;
    info!("Loaded {} part mappings", part_mappings.len());
    trace!("{:?}", part_mappings);

    let load_out_items = match load_out_source {
        Some(source) => load_out::load_items(source),
        None => Ok(vec![]),
    }?;
    info!("Loaded {} load-out items", load_out_items.len());

    let assembly_rules = match assembly_rules_source {
        Some(source) => assembly_rules::load(source),
        None => Ok(vec![]),
    }?;
    info!("Loaded {} assembly rules", assembly_rules.len());

    info!("Assembly variant: {}", assembly_variant.name);
    info!("Ref_des list: {}", assembly_variant.ref_des_list.join(", "));

    let result = AssemblyVariantProcessor::process(&eda_placements, assembly_variant)?;
    let variant_placements = result.placements;
    let variant_placements_count = variant_placements.len();

    info!("Matched {} placements for assembly variant", variant_placements_count);

    trace!("{:?}", part_mappings);

    let processing_result = PartMapper::process(&variant_placements, &part_mappings, &load_out_items, &assembly_rules);

    trace!("{:?}", processing_result);

    let matched_mappings = match &processing_result {
        Ok(mappings) => mappings,
        Err(PartMapperError::MappingErrors(mappings)) => mappings,
    };

    let tree = build_mapping_tree(matched_mappings, eda_substitution_results);
    info!("{}", tree);

    match &processing_result {
        Ok(_) => (),
        Err(PartMapperError::MappingErrors(_)) => {
            error!("Mapping failures")
        }
    }

    write_output_csv(output, matched_mappings)?;

    Ok(())
}

fn write_output_csv(output_file_name: &String, matched_mappings: &Vec<PlacementPartMappingResult>) -> anyhow::Result<()> {

    let output_path = PathBuf::from(output_file_name);

    let mut writer = csv::WriterBuilder::new()
        .quote_style(QuoteStyle::Always)
        .from_path(output_path)?;

    for matched_mapping in matched_mappings.iter() {
        match matched_mapping {
            PlacementPartMappingResult { eda_placement, part, .. } => {

                let empty_value = "".to_string();
                let record = PlacementRecord {
                    ref_des: eda_placement.ref_des.clone(),
                    manufacturer: part.map_or_else(||empty_value.clone(),|part| part.manufacturer.clone()),
                    mpn: part.map_or_else(||empty_value.clone(),|part| part.mpn.clone()),
                    place: eda_placement.place,
                    pcb_side: (&eda_placement.pcb_side).into(),
                    x: eda_placement.x,
                    y: eda_placement.y,
                    rotation: eda_placement.rotation,
                };

                writer.serialize(record)?;
            },
        }
    }

    writer.flush()?;

    Ok(())
}

fn build_mapping_tree(matched_mappings: &Vec<PlacementPartMappingResult>, eda_substitution_results: Vec<EdaSubstitutionResult>) -> Tree<String> {
    let mut tree = Tree::new("Mapping Result".to_string());

    for PlacementPartMappingResult { eda_placement, mapping_result: part_mappings_result, .. } in matched_mappings.iter() {

        fn add_error_node(placement_node: &mut Tree<String>, reason: &str) {
            let placement_error_node = Tree::new(format!("ERROR: Unresolved mapping - {}.", reason).to_string());
            placement_node.leaves.push(placement_error_node);
        }

        if let Some(substitution_result) = eda_substitution_results.iter().find(|candidate|{
            candidate.original_placement.ref_des.eq(&eda_placement.ref_des)
        }) {
            let placement_label = format!("{} ({})", eda_placement.ref_des, EdaPlacementTreeFormatter::format(&substitution_result.original_placement.fields.as_slice()));
            let mut placement_node = Tree::new(placement_label);

            let mut parent = &mut placement_node;

            for chain_entry in substitution_result.chain.iter() {
                let substitution_label = format!("Substituted ({}), by ({})",
                                                 chain_entry.rule.format_transform(),
                                                 chain_entry.rule.format_criteria(),
                );

                let substitution_node = Tree::new(substitution_label);
                parent.leaves.push(substitution_node);
                parent = parent.leaves.last_mut().unwrap();
            }

            match part_mappings_result {
                Ok(part_mapping_results) => {
                    add_mapping_nodes(part_mapping_results, parent);
                }
                Err(PartMappingError::ConflictingRules(part_mapping_results)) => {
                    add_mapping_nodes(part_mapping_results, parent);
                    add_error_node(parent, "Conflicting rules");
                },
                Err(PartMappingError::NoRulesApplied(part_mapping_results)) => {
                    add_mapping_nodes(part_mapping_results, parent);
                    add_error_node(parent, "No rules applied");
                },
                Err(PartMappingError::NoMappings) => {
                    add_error_node(parent, "No mappings found");
                },
            }

            tree.leaves.push(placement_node)
        };

    }

    tree
}

fn add_mapping_nodes(part_mapping_results: &Vec<PartMappingResult>, placement_node: &mut Tree<String>) {
    for PartMappingResult { part_mapping, applied_rule } in part_mapping_results.iter() {
        let part_chunk = format!("manufacturer: '{}', mpn: '{}'", part_mapping.part.manufacturer, part_mapping.part.mpn);
        let mut chunks = vec![part_chunk];

        if let Some(rule) = applied_rule {
            let rule_chunk = format!("({})", rule);
            chunks.push(rule_chunk);
        }

        let part_label = chunks.join(" ");

        let part_node = Tree::new(part_label);
        placement_node.leaves.push(part_node);
    }
}

struct EdaPlacementTreeFormatter {}

impl EdaPlacementTreeFormatter {
    fn format(fields: &[EdaPlacementField]) -> String {
        let chunks: Vec<String> = fields.iter().map(|field|format!("{}: '{}'", field.name, field.value)).collect();
        format!("{}", chunks.join(", "))
    }
}


#[cfg(test)]
mod app_tests {
    use super::*;
    use crux_core::{assert_effect, testing::AppTester};

    #[test]
    fn minimal() {
        let hello = AppTester::<VariantBuilder, _>::default();
        let mut model = Model::default();

        // Call 'update' and request effects
        let update = hello.update(Event::None, &mut model);

        // Check update asked us to `Render`
        assert_effect!(update, Effect::Render(_));

        // Make sure the view matches our expectations
        let actual_view = &hello.view(&model);
        let expected_view = OperationViewModel::default();
        assert_eq!(actual_view, &expected_view);
    }
}
