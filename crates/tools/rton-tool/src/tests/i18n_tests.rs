use super::*;

#[test]
fn resolves_locale_from_preference_before_system_locale() {
    install_test_i18n();

    assert_eq!(resolve_locale(Some("zh-CN"), Some("en-US")), Locale::ZH_CN);
    assert_eq!(resolve_locale(Some("en-US"), Some("zh-CN")), Locale::EN_US);
    assert_eq!(resolve_locale(Some("fr-CA"), Some("en-US")), Locale::FR_FR);
    assert_eq!(
        resolve_locale(Some("ru_RU.UTF-8"), Some("en-US")),
        Locale::RU_RU
    );
    assert_eq!(resolve_locale(Some("es-MX"), Some("en-US")), Locale::ES_ES);
    assert_eq!(resolve_locale(Some("ja-JP"), Some("zh-CN")), Locale::ZH_CN);
    assert_eq!(resolve_locale(None, Some("zh_CN.UTF-8")), Locale::ZH_CN);
    assert_eq!(resolve_locale(None, Some("fr-FR")), Locale::FR_FR);
    assert_eq!(resolve_locale(None, Some("ru-RU")), Locale::RU_RU);
    assert_eq!(resolve_locale(None, Some("es-ES")), Locale::ES_ES);
    assert_eq!(resolve_locale(None, Some("ja-JP")), Locale::EN_US);
}

#[test]
fn renders_added_locale_bundles() {
    install_test_i18n();

    assert_eq!(I18n::new(Locale::FR_FR).t("toolbar-open"), "Ouvrir");
    assert_eq!(I18n::new(Locale::RU_RU).t("toolbar-open"), "Открыть");
    assert_eq!(I18n::new(Locale::ES_ES).t("toolbar-open"), "Abrir");
    for locale in [
        Locale::EN_US,
        Locale::ZH_CN,
        Locale::FR_FR,
        Locale::RU_RU,
        Locale::ES_ES,
    ] {
        assert_eq!(I18n::new(locale).t("toolbar-compact"), "Compact");
    }
}

#[test]
fn language_options_use_native_language_names() {
    install_test_i18n();

    let options = i18n::language_options(I18n::new(Locale::ZH_CN));
    let labels = options
        .iter()
        .map(|option| (option.locale.code(), option.label.as_str()))
        .collect::<Vec<_>>();

    assert!(labels.contains(&("en-US", "English")));
    assert!(labels.contains(&("zh-CN", "中文")));
    assert!(labels.contains(&("fr-FR", "Français")));
    assert!(labels.contains(&("ru-RU", "Русский")));
    assert!(labels.contains(&("es-ES", "Español")));
}

#[test]
fn i18n_keeps_one_bundle_per_locale_and_adds_new_locale() {
    install_test_i18n();

    assert_eq!(I18n::new(Locale::EN_US).t("toolbar-open"), "Open");
    let duplicate_locale =
        i18n::install_locale("en-US", "toolbar-open = Launch\n").expect("valid duplicate");
    assert_eq!(duplicate_locale, Locale::EN_US);
    assert!(i18n::has_locale(Locale::EN_US));
    assert_eq!(I18n::new(Locale::EN_US).t("toolbar-open"), "Open");
    assert_eq!(I18n::new(Locale::EN_US).t("toolbar-compact"), "Compact");

    assert_eq!(
        i18n::locale_code_from_file_name("pt_BR.ftl").as_deref(),
        Some("pt-BR")
    );
    let added_locale =
        i18n::install_locale("de-DE", "language-self = Deutsch\ntoolbar-open = Oeffnen\n")
            .expect("valid added locale");
    assert_eq!(added_locale.code(), "de-DE");
    assert_eq!(Locale::supported_from_code("de-DE"), Some(added_locale));
    assert_eq!(I18n::new(added_locale).t("toolbar-open"), "Oeffnen");
    assert_eq!(I18n::new(added_locale).t("toolbar-compact"), "Compact");
    assert!(
        i18n::language_options(I18n::new(Locale::EN_US))
            .iter()
            .any(|option| option.locale == added_locale && option.label == "Deutsch")
    );

    i18n::clear_locales_for_tests();
}
