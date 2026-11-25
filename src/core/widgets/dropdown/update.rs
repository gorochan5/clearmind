use super::models::{DropdownMessage, DropdownState};

pub fn update(state: &mut DropdownState, message: DropdownMessage) {
    match message {
        DropdownMessage::Toggle => {
            state.is_open = !state.is_open;
            if !state.is_open {
                state.filter.clear();
            }
        }
        DropdownMessage::Close => {
            state.is_open = false;
            state.filter.clear();
        }
        DropdownMessage::Select(value) => {
            state.selected = Some(value);
            state.is_open = false;
            state.filter.clear();
        }
        DropdownMessage::FilterChanged(filter) => {
            state.filter = filter;
        }
    }
}
