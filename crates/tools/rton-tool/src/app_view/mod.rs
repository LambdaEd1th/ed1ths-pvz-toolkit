mod editor_stage;
mod effects;
mod file_panel;
mod handlers;
mod index_panel;
mod layout;
mod page_actions;
mod signals;
mod snapshot;
mod text_selection;
mod workspace;
mod workspace_drop;

pub(crate) use workspace::App;
use workspace::{set_file_sheet_open, set_inspector_sheet_open};
