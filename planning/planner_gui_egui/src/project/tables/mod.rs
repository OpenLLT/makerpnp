use egui_deferred_table::CellIndex;
use tracing::debug;

pub mod load_out;
pub mod parts;
pub mod placements;

//
// Code that probably needs moving into egui_deferred_table
//

#[derive(Debug, Clone)]
enum CellEditState<E, T> {
    /// the pivot point for selections, etc.
    Pivot(CellIndex),
    /// when editing a cell, we need state for the cell and a copy of the original row to be able to track changes
    Editing(CellIndex, E, T),
}

trait ApplyChange<T, E> {
    fn apply_change(&mut self, value: T) -> Result<(), E>;
}

/// Implement this to enable data source editing support.
trait EditableTableRenderer<DataSource> {
    /// Usually a type containing the data for a single row.
    type Value;
    /// Usually an enum, with variants for each type of cell that can be edited.
    /// e.g. `Number(f32)`, `Text(String)`...
    type ItemState;

    /// Called when the cell needs to be edited.
    ///
    /// Return None to prevent editing or a tuple containing the ItemState and the original value.
    fn build_item_state(
        &self,
        cell_index: CellIndex,
        source: &mut DataSource,
    ) -> Option<(Self::ItemState, Self::Value)>;

    /// Called when the cell is no-longer being edited.
    ///
    /// Implementations usually modify the data source directly, or build and send a command that will change
    /// eventually update the datasource, e.g. in a background thread.
    fn on_edit_complete(
        &mut self,
        index: CellIndex,
        state: Self::ItemState,
        original_item: Self::Value,
        source: &mut DataSource,
    );

    // The data source needs to own a `CellEditState`, the following three methods are used to modify it.
    // typically the data source just has a member like this: `cell: Option<CellEditState<MyItemState, MyRow>>`

    fn set_edit_state(&mut self, edit_state: CellEditState<Self::ItemState, Self::Value>);
    fn edit_state(&self) -> Option<&CellEditState<Self::ItemState, Self::Value>>;
    fn take_edit_state(&mut self) -> CellEditState<Self::ItemState, Self::Value>;
}

fn handle_cell_click<E, S, R: EditableTableRenderer<S, Value = T, ItemState = E>, T: Clone>(
    source: &mut S,
    renderer: &mut R,
    cell_index: CellIndex,
) {
    match renderer.edit_state() {
        None => {
            // change selection
            renderer.set_edit_state(CellEditState::Pivot(cell_index));
        }
        Some(CellEditState::Pivot(pivot_cell_index)) if *pivot_cell_index == cell_index => {
            debug!("clicked in selected cell");

            // change mode to edit
            let edit_state = renderer.build_item_state(cell_index, source);
            if let Some((edit, original_item)) = edit_state {
                renderer.set_edit_state(CellEditState::Editing(cell_index, edit, original_item));
            }
        }
        Some(CellEditState::Pivot(_)) => {
            debug!("clicked in different cell");

            // change selection
            renderer.set_edit_state(CellEditState::Pivot(cell_index));
        }
        Some(CellEditState::Editing(editing_cell_index, _cell_edit_state, _original_item))
            if *editing_cell_index == cell_index =>
        {
            debug!("clicked in cell while editing");

            // nothing to do
        }
        Some(CellEditState::Editing(_editing_cell_index, _cell_edit_state, _original_item)) => {
            debug!("clicked in a different cell while editing");

            // apply edited value
            let CellEditState::Editing(index, state, original_item) = renderer.take_edit_state() else {
                unreachable!();
            };
            renderer.on_edit_complete(index, state, original_item, source);

            // change selection
            renderer.set_edit_state(CellEditState::Pivot(cell_index));
        }
    }
}
