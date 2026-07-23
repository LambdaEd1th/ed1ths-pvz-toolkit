#[cfg(target_arch = "wasm32")]
use dioxus::prelude::*;

use crate::i18n::{self, Locale};
use crate::platform;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn load_i18n_sources() -> usize {
    platform::read_i18n_sources()
        .into_iter()
        .filter(|source| install_locale_source(&source.code, &source.source))
        .count()
}

#[cfg(all(test, target_arch = "wasm32"))]
pub(crate) fn load_i18n_sources() -> usize {
    [
        ("en-US", include_str!("../assets/rton/i18n/en-US.ftl")),
        ("es-ES", include_str!("../assets/rton/i18n/es-ES.ftl")),
        ("fr-FR", include_str!("../assets/rton/i18n/fr-FR.ftl")),
        ("ru-RU", include_str!("../assets/rton/i18n/ru-RU.ftl")),
        ("zh-CN", include_str!("../assets/rton/i18n/zh-CN.ftl")),
    ]
    .into_iter()
    .filter(|(code, source)| install_locale_source(code, source))
    .count()
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn load_i18n_sources_async() -> usize {
    platform::read_i18n_sources_async()
        .await
        .into_iter()
        .filter(|source| install_locale_source(&source.code, &source.source))
        .count()
}

fn install_locale_source(code: &str, source: &str) -> bool {
    match i18n::install_locale(code, source) {
        Ok(_) => true,
        Err(error) => {
            log::warn!(target: "rton_editor::i18n", "Failed to load locale {code}: {error}");
            false
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn bump_i18n_revision(mut i18n_revision: Signal<u64>) {
    let next_revision = *i18n_revision.read() + 1;
    i18n_revision.set(next_revision);
}

pub(crate) fn initial_locale() -> Locale {
    resolve_locale(
        platform::read_locale_preference().as_deref(),
        platform::system_locale().as_deref(),
    )
}

pub(crate) fn resolve_locale(preference: Option<&str>, system_locale: Option<&str>) -> Locale {
    preference
        .and_then(Locale::supported_from_code)
        .or_else(|| system_locale.and_then(Locale::supported_from_code))
        .unwrap_or(Locale::EN_US)
}
