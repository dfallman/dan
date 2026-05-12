//! Command palette: fuzzy-search across actions, buffers, and project files.

pub mod match_;
pub mod items;
pub mod index;
pub mod state;

#[allow(unused_imports)]
pub use state::PaletteState;
#[allow(unused_imports)]
pub use items::{PaletteItem, ActionId, action_registry, action_to_command};
