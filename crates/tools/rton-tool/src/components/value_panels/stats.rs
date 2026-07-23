use dioxus::prelude::*;
use rton_editor_core::ValueStats;

use crate::i18n::I18n;

#[component]
pub(crate) fn StatsGrid(stats: ValueStats, i18n: I18n) -> Element {
    rsx! {
        div { class: "stats-grid",
            StatCell { label: i18n.t("stats-nodes"), value: stats.nodes }
            StatCell { label: i18n.t("stats-objects"), value: stats.objects }
            StatCell { label: i18n.t("stats-arrays"), value: stats.arrays }
            StatCell { label: i18n.t("stats-depth"), value: stats.max_depth }
        }
    }
}

#[component]
fn StatCell(label: String, value: usize) -> Element {
    rsx! {
        div { class: "stat-cell",
            span { "{label}" }
            strong { "{value}" }
        }
    }
}
