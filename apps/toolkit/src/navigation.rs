use dioxus::prelude::*;

use crate::i18n::I18n;
use crate::tool_registry::localized_label;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AppRoute {
    #[default]
    Home,
    Libraries,
    About,
    Rsb,
    RsbPatch,
    Pak,
    Dzip,
    Smf,
    CompiledText,
    CryptData,
    Rton,
    Pam,
    Particle,
    Reanim,
    Wem,
    Bnk,
    Newton,
}

impl AppRoute {
    pub(crate) fn label(self, i18n: I18n) -> String {
        match self {
            Self::Home => i18n.t("nav-home"),
            Self::Libraries => i18n.t("nav-libraries"),
            Self::About => i18n.t("nav-about"),
            Self::Rsb => localized_label(i18n, "rsb", "RSB Archive"),
            Self::RsbPatch => localized_label(i18n, "rsb-patch", "RSB Patch"),
            Self::Pak => localized_label(i18n, "pak", "PAK Archive"),
            Self::Dzip => localized_label(i18n, "dzip", "DZip Archive"),
            Self::Smf => localized_label(i18n, "smf", "SMF Container"),
            Self::CompiledText => localized_label(i18n, "compiled-text", "Compiled Text"),
            Self::CryptData => localized_label(i18n, "crypt-data", "Crypt-Data"),
            Self::Rton => localized_label(i18n, "rton", "RTON Editor"),
            Self::Pam => localized_label(i18n, "pam", "PAM Editor"),
            Self::Particle => localized_label(i18n, "particle", "Particle Editor"),
            Self::Reanim => localized_label(i18n, "reanim", "REANIM Editor"),
            Self::Wem => localized_label(i18n, "wem", "WEM Audio"),
            Self::Bnk => localized_label(i18n, "bnk", "BNK Archive"),
            Self::Newton => localized_label(i18n, "newton", "NEWTON Manifest"),
        }
    }
}

pub(crate) fn navigate(
    mut route: Signal<AppRoute>,
    mut sidebar_open: Signal<bool>,
    compact_shell: bool,
    target: AppRoute,
) {
    route.set(target);
    if compact_shell
        || matches!(
            target,
            AppRoute::Rsb
                | AppRoute::RsbPatch
                | AppRoute::Pak
                | AppRoute::Dzip
                | AppRoute::Smf
                | AppRoute::CompiledText
                | AppRoute::CryptData
                | AppRoute::Rton
                | AppRoute::Pam
                | AppRoute::Particle
                | AppRoute::Reanim
                | AppRoute::Wem
                | AppRoute::Bnk
                | AppRoute::Newton
        )
    {
        sidebar_open.set(false);
    }
}
