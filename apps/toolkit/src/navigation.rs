use dioxus::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum AppRoute {
    #[default]
    Home,
    Pam,
    Rton,
}

impl AppRoute {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Pam => "PAM Viewer",
            Self::Rton => "RTON Editor",
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
    if compact_shell || target != AppRoute::Home {
        sidebar_open.set(false);
    }
}
