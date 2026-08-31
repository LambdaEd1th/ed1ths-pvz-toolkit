mod app;
mod i18n;
mod i18n_sources;
mod library_registry;
mod navigation;
mod pages;
#[cfg(not(target_arch = "wasm32"))]
mod preferences;
mod shell;
mod startup;
mod tool_registry;
mod update_check;

fn main() {
    app::launch();
}
