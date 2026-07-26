//! Shared content pieces for the RTON editor page.

use dioxus::prelude::*;
use dioxus_free_icons::icons::ld_icons::{LdActivity, LdFileArchive, LdFolderOpen};
use rton_editor_core::ValueStats;

use crate::app_constants::LOADABLE_FILE_HINT;
use crate::components::{MetaItem, PanelHeader, StatsGrid, lucide_icon};
use crate::i18n::I18n;

#[component]
pub(crate) fn EmptyDropStage(i18n: I18n) -> Element {
    rsx! {
        div { class: "rton-empty-drop-stage",
            div { class: "empty-editor-icon", {lucide_icon(LdFolderOpen)} }
            div { class: "empty-editor-title", {i18n.t("drop-title")} }
            div {
                class: "empty-editor-subtitle",
                {i18n.t_args("drop-subtitle", &[("hint", LOADABLE_FILE_HINT.to_string())])}
            }
            div { class: "rton-format-row", aria_hidden: "true",
                span { "RTON" }
                span { "JSON" }
                span { "YAML" }
                span { "TOML" }
            }
        }
    }
}

#[component]
pub(crate) fn FileSummaryPanel(
    i18n: I18n,
    active_file_label: String,
    input_value: String,
    output_value: String,
    stats: Option<ValueStats>,
) -> Element {
    rsx! {
        div { class: "rton-inspector-summary",
            PanelHeader {
                icon: lucide_icon(LdFileArchive),
                title: i18n.t("panel-file-properties"),
                subtitle: i18n.t("panel-current-file")
            }
            section { class: "file-summary-meta-section",
                dl { class: "meta-list",
                    MetaItem {
                        label: i18n.t("panel-name"),
                        value: active_file_label
                    }
                    MetaItem {
                        label: i18n.t("panel-input"),
                        value: input_value
                    }
                    MetaItem {
                        label: i18n.t("panel-output"),
                        value: output_value
                    }
                }
            }
            if let Some(stats) = stats {
                section { class: "file-summary-stats-section",
                    div { class: "stats-title",
                        span { class: "panel-header-icon muted", {lucide_icon(LdActivity)} }
                        h2 { {i18n.t("panel-stats")} }
                    }
                    StatsGrid { stats, i18n }
                }
            }
        }
    }
}
