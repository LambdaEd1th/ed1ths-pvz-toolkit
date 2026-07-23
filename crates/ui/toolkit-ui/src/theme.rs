use dioxus::prelude::*;

const APPEARANCE_STORAGE_KEY: &str = "ed1ths-pvz-toolkit-appearance";
const LOAD_APPEARANCE: &str = r#"
try {
    dioxus.send(localStorage.getItem("ed1ths-pvz-toolkit-appearance") ?? "system");
} catch (_) {
    dioxus.send("system");
}
"#;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    pub fn from_code(code: &str) -> Self {
        match code {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::System,
        }
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub const fn class(self) -> &'static str {
        match self {
            Self::System => "tk-theme-system system-theme",
            Self::Light => "tk-theme-light light-theme",
            Self::Dark => "tk-theme-dark dark-theme",
        }
    }
}

#[derive(Clone, Copy)]
pub struct AppearanceContext {
    preference: Signal<Appearance>,
    hydrated: Signal<bool>,
}

impl AppearanceContext {
    pub fn preference(self) -> Appearance {
        (self.preference)()
    }

    pub fn set(self, appearance: Appearance) {
        let mut preference = self.preference;
        preference.set(appearance);
    }

    pub fn hydrated(self) -> bool {
        (self.hydrated)()
    }
}

pub fn use_appearance() -> AppearanceContext {
    use_context::<AppearanceContext>()
}

#[component]
pub fn AppearanceProvider(children: Element) -> Element {
    let preference = use_signal(Appearance::default);
    let hydrated = use_signal(|| false);
    let context = AppearanceContext {
        preference,
        hydrated,
    };
    use_context_provider(|| context);

    let mut loaded_preference = preference;
    let mut appearance_ready = hydrated;
    use_effect(move || {
        let mut evaluator = document::eval(LOAD_APPEARANCE);
        spawn(async move {
            if let Ok(code) = evaluator.recv::<String>().await {
                loaded_preference.set(Appearance::from_code(&code));
            }
            appearance_ready.set(true);
        });
    });

    use_effect(use_reactive(
        &(preference(), hydrated()),
        move |(appearance, ready)| {
            if !ready {
                return;
            }
            let script = format!(
                "try {{ localStorage.setItem('{APPEARANCE_STORAGE_KEY}', '{}'); }} catch (_) {{}}",
                appearance.code()
            );
            let _ = document::eval(&script);
        },
    ));

    rsx! { {children} }
}
