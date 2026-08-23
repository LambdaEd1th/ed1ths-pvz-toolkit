use std::{cell::RefCell, collections::HashMap};

use dioxus::prelude::*;
use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

use crate::i18n_sources;

thread_local! {
    static LOCALE_BUNDLES: RefCell<HashMap<&'static str, FluentBundle<FluentResource>>> = RefCell::new(HashMap::new());
    static INTERNED_LOCALE_CODES: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Locale {
    code: &'static str,
}

impl Locale {
    pub(crate) const EN_US: Self = Self { code: "en-US" };
    pub(crate) const ZH_CN: Self = Self { code: "zh-CN" };
    pub(crate) const FR_FR: Self = Self { code: "fr-FR" };
    pub(crate) const RU_RU: Self = Self { code: "ru-RU" };
    pub(crate) const ES_ES: Self = Self { code: "es-ES" };

    pub(crate) fn supported_from_code(code: &str) -> Option<Self> {
        let normalized = canonical_locale_code(code)?;
        installed_locale_from_exact(&normalized)
            .or_else(|| installed_locale_from_language(&normalized))
    }

    pub(crate) const fn code(self) -> &'static str {
        self.code
    }

    fn constant_from_exact(code: &str) -> Option<Self> {
        [
            Self::EN_US,
            Self::ZH_CN,
            Self::FR_FR,
            Self::RU_RU,
            Self::ES_ES,
        ]
        .into_iter()
        .find(|locale| locale.code.eq_ignore_ascii_case(code))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct I18n {
    locale: Locale,
}

impl I18n {
    pub(crate) const fn new(locale: Locale) -> Self {
        Self { locale }
    }

    pub(crate) fn t(self, key: &str) -> String {
        self.t_args(key, &[])
    }

    pub(crate) fn t_args(self, key: &str, args: &[(&str, String)]) -> String {
        render(self.locale, key, args)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LanguageOption {
    pub(crate) locale: Locale,
    pub(crate) label: String,
}

#[derive(Clone, Copy)]
pub(crate) struct LocaleContext {
    locale: Signal<Locale>,
    revision: Signal<u64>,
}

impl LocaleContext {
    pub(crate) fn locale(self) -> Locale {
        (self.locale)()
    }

    pub(crate) fn set(self, locale: Locale) {
        let mut current = self.locale;
        current.set(locale);
        let _ = i18n_sources::save_locale_preference(locale.code());
    }
}

pub(crate) fn use_locale() -> LocaleContext {
    use_context::<LocaleContext>()
}

pub(crate) fn use_i18n() -> I18n {
    let context = use_locale();
    let _revision = (context.revision)();
    I18n::new(context.locale())
}

#[component]
pub(crate) fn LocaleProvider(children: Element) -> Element {
    use_hook(install_builtin_locales);
    #[cfg(not(target_arch = "wasm32"))]
    use_hook(load_external_locales);

    let initial_locale = resolve_locale(
        i18n_sources::read_locale_preference().as_deref(),
        i18n_sources::system_locale().as_deref(),
    );
    let locale = use_signal(move || initial_locale);
    let revision = use_signal(|| 0_u64);
    use_context_provider(|| LocaleContext { locale, revision });

    #[cfg(target_arch = "wasm32")]
    {
        let mut revision = revision;
        use_effect(move || {
            spawn(async move {
                if i18n_sources::read_i18n_sources_async().await > 0 {
                    revision.set(revision().wrapping_add(1));
                }
            });
        });
    }

    use_effect(use_reactive(&locale(), move |locale| {
        let script = format!(
            "document.documentElement.lang = {};",
            serde_json::to_string(locale.code()).unwrap_or_else(|_| "\"en-US\"".to_string())
        );
        let _ = document::eval(&script);
    }));

    rsx! { {children} }
}

pub(crate) fn language_options(i18n: I18n) -> Vec<LanguageOption> {
    available_locales()
        .into_iter()
        .map(|locale| LanguageOption {
            locale,
            label: format_locale_message(locale, "language-self", &[])
                .or_else(|| format_locale_message(locale, "language-name", &[]))
                .unwrap_or_else(|| i18n.t(locale.code())),
        })
        .collect()
}

pub(crate) fn install_locale(code: &str, source: &str) -> Result<Locale, String> {
    let code = canonical_locale_code(code).ok_or_else(|| "invalid locale code".to_string())?;
    let langid: LanguageIdentifier = code
        .parse()
        .map_err(|error| format!("invalid locale identifier: {error}"))?;
    let resource = FluentResource::try_new(source.to_string())
        .map_err(|(_, errors)| format!("invalid FTL: {errors:?}"))?;
    let mut bundle = FluentBundle::new(vec![langid]);
    bundle
        .add_resource(resource)
        .map_err(|errors| format!("invalid FTL resource: {errors:?}"))?;
    let locale = intern_locale_code(&code);
    LOCALE_BUNDLES.with(|bundles| {
        bundles.borrow_mut().entry(locale.code()).or_insert(bundle);
    });
    Ok(locale)
}

fn install_builtin_locales() {
    for (code, source) in [
        ("en-US", include_str!("../assets/i18n/en-US.ftl")),
        ("es-ES", include_str!("../assets/i18n/es-ES.ftl")),
        ("fr-FR", include_str!("../assets/i18n/fr-FR.ftl")),
        ("ru-RU", include_str!("../assets/i18n/ru-RU.ftl")),
        ("zh-CN", include_str!("../assets/i18n/zh-CN.ftl")),
    ] {
        let _ = install_locale(code, source);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn load_external_locales() {
    for source in i18n_sources::read_i18n_sources() {
        let _ = install_locale(&source.code, &source.source);
    }
}

fn resolve_locale(preference: Option<&str>, system_locale: Option<&str>) -> Locale {
    preference
        .and_then(Locale::supported_from_code)
        .or_else(|| system_locale.and_then(Locale::supported_from_code))
        .unwrap_or(Locale::EN_US)
}

fn available_locales() -> Vec<Locale> {
    let mut locales = LOCALE_BUNDLES.with(|bundles| {
        bundles
            .borrow()
            .keys()
            .copied()
            .map(|code| Locale { code })
            .collect::<Vec<_>>()
    });
    locales.sort_by(|left, right| left.code().cmp(right.code()));
    locales
}

fn render(locale: Locale, key: &str, args: &[(&str, String)]) -> String {
    format_locale_message(locale, key, args)
        .or_else(|| {
            (locale != Locale::EN_US)
                .then(|| format_locale_message(Locale::EN_US, key, args))
                .flatten()
        })
        .unwrap_or_else(|| key.to_string())
}

fn format_locale_message(locale: Locale, key: &str, args: &[(&str, String)]) -> Option<String> {
    LOCALE_BUNDLES.with(|bundles| {
        bundles.borrow().get(locale.code()).and_then(|bundle| {
            let message = bundle.get_message(key)?;
            let pattern = message.value()?;
            let mut fluent_args = FluentArgs::new();
            for (name, value) in args {
                fluent_args.set(*name, value.as_str());
            }
            let mut errors = Vec::new();
            Some(
                bundle
                    .format_pattern(pattern, Some(&fluent_args), &mut errors)
                    .into_owned(),
            )
        })
    })
}

fn installed_locale_from_exact(code: &str) -> Option<Locale> {
    LOCALE_BUNDLES.with(|bundles| {
        bundles
            .borrow()
            .keys()
            .copied()
            .find(|candidate| candidate.eq_ignore_ascii_case(code))
            .map(|code| Locale { code })
    })
}

fn installed_locale_from_language(code: &str) -> Option<Locale> {
    let language = code.split('-').next().unwrap_or(code);
    let mut matches = LOCALE_BUNDLES.with(|bundles| {
        bundles
            .borrow()
            .keys()
            .copied()
            .filter(|candidate| {
                candidate
                    .split('-')
                    .next()
                    .unwrap_or(candidate)
                    .eq_ignore_ascii_case(language)
            })
            .collect::<Vec<_>>()
    });
    matches.sort_unstable();
    matches.first().copied().map(|code| Locale { code })
}

fn intern_locale_code(code: &str) -> Locale {
    if let Some(locale) = Locale::constant_from_exact(code) {
        return locale;
    }
    INTERNED_LOCALE_CODES.with(|codes| {
        let mut codes = codes.borrow_mut();
        if let Some(code) = codes
            .iter()
            .copied()
            .find(|candidate| candidate.eq_ignore_ascii_case(code))
        {
            return Locale { code };
        }
        let code = Box::leak(code.to_string().into_boxed_str());
        codes.push(code);
        Locale { code }
    })
}

fn canonical_locale_code(code: &str) -> Option<String> {
    let normalized = code
        .trim()
        .split(['.', ':'])
        .next()
        .unwrap_or(code)
        .replace('_', "-");
    let parts = normalized
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    let language = *parts.first()?;
    if !(2..=8).contains(&language.len()) || !language.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return None;
    }
    let mut canonical = vec![language.to_ascii_lowercase()];
    for part in parts.iter().skip(1) {
        if !(1..=8).contains(&part.len()) || !part.chars().all(|ch| ch.is_ascii_alphanumeric()) {
            return None;
        }
        if part.len() == 2 && part.chars().all(|ch| ch.is_ascii_alphabetic()) {
            canonical.push(part.to_ascii_uppercase());
        } else if part.len() == 4 && part.chars().all(|ch| ch.is_ascii_alphabetic()) {
            let mut chars = part.chars();
            canonical.push(format!(
                "{}{}",
                chars.next()?.to_ascii_uppercase(),
                chars.as_str().to_ascii_lowercase()
            ));
        } else {
            canonical.push(part.to_ascii_lowercase());
        }
    }
    Some(canonical.join("-"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_exact_and_language_only_locales() {
        install_builtin_locales();
        assert_eq!(available_locales().len(), 5);
        assert_eq!(resolve_locale(Some("zh_CN.UTF-8"), None), Locale::ZH_CN);
        assert_eq!(resolve_locale(None, Some("fr-CA")), Locale::FR_FR);
        assert_eq!(resolve_locale(None, Some("ja-JP")), Locale::EN_US);
    }

    #[test]
    fn falls_back_to_english_for_missing_messages() {
        install_builtin_locales();
        assert_eq!(
            I18n::new(Locale::ZH_CN).t("app-name"),
            "Ed1th's PvZ Toolkit"
        );
    }
}
