use dioxus::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AppRoute {
    #[default]
    Home,
    About,
    Rsb,
    Rton,
    Pam,
    Wem,
    Newton,
}

impl AppRoute {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::About => "About",
            Self::Rsb => "RSB Archive",
            Self::Rton => "RTON Editor",
            Self::Pam => "PAM Viewer",
            Self::Wem => "WEM Audio",
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
            AppRoute::Rsb | AppRoute::Rton | AppRoute::Pam | AppRoute::Wem | AppRoute::Newton
        )
    {
        sidebar_open.set(false);
    }
}
