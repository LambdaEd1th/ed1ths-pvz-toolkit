use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{
    LdCamera, LdClapperboard, LdFileArchive, LdFileCode2, LdFileImage, LdFileJson, LdFilm,
    LdRefreshCw,
};

use crate::actions::{ExportKind, start_export};
use crate::state::AppContext;

use super::super::primitives::icon;

#[component]
pub(super) fn ExportGroup(on_dismiss: EventHandler<()>) -> Element {
    let context = use_context::<AppContext>();
    let disabled = context.active_tab.read().is_none() || context.export.read().is_some();
    rsx! {
        ExportButton { label: "PNG", disabled, icon_element: icon(LdCamera), kind: ExportKind::Png, on_dismiss }
        ExportButton { label: "APNG", disabled, icon_element: icon(LdFileImage), kind: ExportKind::Apng, on_dismiss }
        ExportButton { label: "WebP", disabled, icon_element: icon(LdFilm), kind: ExportKind::Webp, on_dismiss }
        ExportButton { label: "FLA", disabled, icon_element: icon(LdClapperboard), kind: ExportKind::Fla, on_dismiss }
    }
}

#[component]
pub(super) fn ConvertGroup(on_dismiss: EventHandler<()>) -> Element {
    let context = use_context::<AppContext>();
    let disabled = context.active_tab.read().is_none() || context.export.read().is_some();
    rsx! {
        ExportButton { label: "JSON", disabled, icon_element: icon(LdFileJson), kind: ExportKind::Json, on_dismiss }
        ExportButton { label: "YAML", disabled, icon_element: icon(LdFileCode2), kind: ExportKind::Yaml, on_dismiss }
        ExportButton { label: "TOML", disabled, icon_element: icon(LdFileArchive), kind: ExportKind::Toml, on_dismiss }
        ExportButton { label: "PAM", disabled, icon_element: icon(LdRefreshCw), kind: ExportKind::Pam, on_dismiss }
    }
}

#[component]
fn ExportButton(
    label: &'static str,
    disabled: bool,
    icon_element: Element,
    kind: ExportKind,
    on_dismiss: EventHandler<()>,
) -> Element {
    let context = use_context::<AppContext>();
    rsx! {
        button {
            r#type: "button", class: "pam-button", disabled, title: "{label}",
            onclick: move |_| {
                on_dismiss.call(());
                start_export(context, kind);
            },
            {icon_element} span { "{label}" }
        }
    }
}
