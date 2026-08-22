use dioxus::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AppRoute {
    #[default]
    Home,
    About,
    Rsb,
    RsbPatch,
    Pak,
    Dzip,
    Smf,
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
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::About => "About",
            Self::Rsb => "RSB Archive",
            Self::RsbPatch => "RSB Patch",
            Self::Pak => "PAK Archive",
            Self::Dzip => "DZip Archive",
            Self::Smf => "SMF Container",
            Self::CryptData => "Crypt-Data",
            Self::Rton => "RTON Editor",
            Self::Pam => "PAM Viewer",
            Self::Particle => "Particle Editor",
            Self::Reanim => "REANIM Editor",
            Self::Wem => "WEM Audio",
            Self::Bnk => "BNK Archive",
            Self::Newton => "NEWTON Manifest",
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
