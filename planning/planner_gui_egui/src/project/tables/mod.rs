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

trait EditableDataSource {
    type Value;
    type ItemState;
    type EditState;

    fn build_edit_state(&self, cell_index: CellIndex) -> Option<(Self::ItemState, Self::Value)>;
    fn on_edit_complete(&mut self, index: CellIndex, state: Self::ItemState, original_item: Self::Value);

    fn set_edit_state(&mut self, edit_state: Self::EditState);
    fn edit_state(&self) -> Option<&Self::EditState>;
    fn take_state(&mut self) -> Self::EditState;
}

fn handle_cell_click<E, S: EditableDataSource<EditState = CellEditState<E, T>, Value = T, ItemState = E>, T: Clone>(
    data_source: &mut S,
    cell_index: CellIndex,
) {
    match data_source.edit_state() {
        None => {
            // change selection
            data_source.set_edit_state(CellEditState::Pivot(cell_index));
        }
        Some(CellEditState::Pivot(pivot_cell_index)) if *pivot_cell_index == cell_index => {
            debug!("clicked in selected cell");

            // change mode to edit
            let edit_state = data_source.build_edit_state(cell_index);
            if let Some((edit, original_item)) = edit_state {
                data_source.set_edit_state(CellEditState::Editing(cell_index, edit, original_item));
            }
        }
        Some(CellEditState::Pivot(_)) => {
            debug!("clicked in different cell");

            // change selection
            data_source.set_edit_state(CellEditState::Pivot(cell_index));
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
            let CellEditState::Editing(index, state, original_item) = data_source.take_state() else {
                unreachable!();
            };
            data_source.on_edit_complete(index, state, original_item);

            // change selection
            data_source.set_edit_state(CellEditState::Pivot(cell_index));
        }
    }
}
