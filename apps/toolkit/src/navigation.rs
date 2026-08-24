use dioxus::prelude::*;

use crate::i18n::I18n;

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
            Self::Rsb => "RSB Archive".to_string(),
            Self::RsbPatch => "RSB Patch".to_string(),
            Self::Pak => "PAK Archive".to_string(),
            Self::Dzip => "DZip Archive".to_string(),
            Self::Smf => "SMF Container".to_string(),
            Self::CompiledText => "Compiled Text".to_string(),
            Self::CryptData => "Crypt-Data".to_string(),
            Self::Rton => "RTON Editor".to_string(),
            Self::Pam => "PAM Viewer".to_string(),
            Self::Particle => "Particle Editor".to_string(),
            Self::Reanim => "REANIM Editor".to_string(),
            Self::Wem => "WEM Audio".to_string(),
            Self::Bnk => "BNK Archive".to_string(),
            Self::Newton => "NEWTON Manifest".to_string(),
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
