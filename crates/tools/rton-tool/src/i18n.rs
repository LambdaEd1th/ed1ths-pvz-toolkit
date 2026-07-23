use std::{cell::RefCell, collections::HashMap};

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

thread_local! {
    static LOCALE_BUNDLES: RefCell<HashMap<&'static str, FluentBundle<FluentResource>>> = RefCell::new(HashMap::new());
    static INTERNED_LOCALE_CODES: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Locale {
    code: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageOption {
    pub locale: Locale,
    pub label: String,
}

impl Locale {
    pub const EN_US: Self = Self { code: "en-US" };
    pub const ZH_CN: Self = Self { code: "zh-CN" };
    pub const FR_FR: Self = Self { code: "fr-FR" };
    pub const RU_RU: Self = Self { code: "ru-RU" };
    pub const ES_ES: Self = Self { code: "es-ES" };
    pub fn supported_from_code(code: &str) -> Option<Self> {
        let normalized = canonical_locale_code(code)?;
        installed_locale_from_exact(&normalized)
            .or_else(|| installed_locale_from_language(&normalized))
    }

    pub fn code(self) -> &'static str {
        self.code
    }

    fn constant_from_exact(code: &str) -> Option<Self> {
        if code.eq_ignore_ascii_case(Self::EN_US.code) {
            Some(Self::EN_US)
        } else if code.eq_ignore_ascii_case(Self::ZH_CN.code) {
            Some(Self::ZH_CN)
        } else if code.eq_ignore_ascii_case(Self::FR_FR.code) {
            Some(Self::FR_FR)
        } else if code.eq_ignore_ascii_case(Self::RU_RU.code) {
            Some(Self::RU_RU)
        } else if code.eq_ignore_ascii_case(Self::ES_ES.code) {
            Some(Self::ES_ES)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct I18n {
    locale: Locale,
}

impl I18n {
    pub fn new(locale: Locale) -> Self {
        Self { locale }
    }

    pub fn t(self, key: &str) -> String {
        self.t_args(key, &[])
    }

    pub fn t_args(self, key: &str, args: &[(&str, String)]) -> String {
        render(self.locale, key, args)
    }
}

#[cfg(test)]
pub fn locale_code_from_file_name(name: &str) -> Option<String> {
    let file_name = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let stem = file_name.strip_suffix(".ftl").unwrap_or(file_name);
    canonical_locale_code(stem)
}

pub fn install_locale(code: &str, source: &str) -> Result<Locale, String> {
    let code = canonical_locale_code(code).ok_or_else(|| "invalid locale code".to_string())?;
    let bundle = build_bundle_from_source(&code, source)?;
    let locale = intern_locale_code(&code);
    LOCALE_BUNDLES.with(|bundles| {
        bundles.borrow_mut().entry(locale.code()).or_insert(bundle);
    });
    Ok(locale)
}

#[cfg(test)]
pub fn has_locale(locale: Locale) -> bool {
    LOCALE_BUNDLES.with(|bundles| bundles.borrow().contains_key(locale.code()))
}

pub fn available_locales() -> Vec<Locale> {
    let mut locales = LOCALE_BUNDLES.with(|bundles| {
        bundles
            .borrow()
            .keys()
            .copied()
            .map(|code| Locale { code })
            .collect::<Vec<_>>()
    });
    locales.sort_by(|left, right| left.code().cmp(right.code()));
    if locales.is_empty() {
        locales.push(Locale::EN_US);
    }
    locales
}

pub fn language_options(i18n: I18n) -> Vec<LanguageOption> {
    available_locales()
        .into_iter()
        .map(|locale| LanguageOption {
            locale,
            label: language_label(locale, i18n),
        })
        .collect()
}

#[cfg(test)]
pub fn clear_locales_for_tests() {
    LOCALE_BUNDLES.with(|bundles| bundles.borrow_mut().clear());
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
        bundles
            .borrow()
            .get(locale.code())
            .and_then(|bundle| format_message(bundle, key, args))
    })
}

fn build_bundle_from_source(
    locale_code: &str,
    source: &str,
) -> Result<FluentBundle<FluentResource>, String> {
    let langid: LanguageIdentifier = locale_code
        .parse()
        .map_err(|error| format!("invalid locale identifier: {error}"))?;
    let resource = FluentResource::try_new(source.to_string())
        .map_err(|(_, errors)| format!("invalid FTL: {errors:?}"))?;
    let mut bundle = FluentBundle::new(vec![langid]);
    bundle
        .add_resource(resource)
        .map_err(|errors| format!("invalid FTL resource: {errors:?}"))?;
    Ok(bundle)
}

fn format_message(
    bundle: &FluentBundle<FluentResource>,
    key: &str,
    args: &[(&str, String)],
) -> Option<String> {
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
    let language = code.split('-').next().unwrap_or(code).to_ascii_lowercase();
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
                    .eq_ignore_ascii_case(&language)
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
        if let Some(existing) = codes
            .iter()
            .copied()
            .find(|candidate| candidate.eq_ignore_ascii_case(code))
        {
            return Locale { code: existing };
        }

        let leaked = Box::leak(code.to_string().into_boxed_str());
        codes.push(leaked);
        Locale { code: leaked }
    })
}

fn language_label(locale: Locale, _i18n: I18n) -> String {
    format_locale_message(locale, "language-self", &[])
        .or_else(|| format_locale_message(locale, "language-name", &[]))
        .unwrap_or_else(|| locale.code().to_string())
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

    if parts.is_empty() {
        return None;
    }

    let language = parts[0];
    if !(2..=8).contains(&language.len()) || !language.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return None;
    }

    let mut canonical = Vec::with_capacity(parts.len());
    canonical.push(language.to_ascii_lowercase());
    for part in parts.iter().skip(1) {
        if !(1..=8).contains(&part.len()) || !part.chars().all(|ch| ch.is_ascii_alphanumeric()) {
            return None;
        }

        if part.len() == 2 && part.chars().all(|ch| ch.is_ascii_alphabetic()) {
            canonical.push(part.to_ascii_uppercase());
        } else if part.len() == 4 && part.chars().all(|ch| ch.is_ascii_alphabetic()) {
            let mut chars = part.chars();
            let first = chars.next()?.to_ascii_uppercase();
            let rest = chars.as_str().to_ascii_lowercase();
            canonical.push(format!("{first}{rest}"));
        } else {
            canonical.push(part.to_ascii_lowercase());
        }
    }

    Some(canonical.join("-"))
}
