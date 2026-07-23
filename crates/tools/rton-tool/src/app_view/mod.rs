mod editor_stage;
mod effects;
mod file_panel;
mod handlers;
mod index_panel;
mod layout;
mod signals;
mod snapshot;
mod text_selection;
mod toolbar_view;
mod workspace;
mod workspace_drop;
mod workspace_frame;

pub(crate) use workspace::App;
use workspace::{set_file_drawer_visibility, set_inspector_drawer_visibility};
