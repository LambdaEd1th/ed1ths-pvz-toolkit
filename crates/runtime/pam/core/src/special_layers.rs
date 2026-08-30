use std::collections::HashSet;

use pam_codec::PamInfo;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpecialLayerIndices {
    pub plant_custom_layers: Vec<usize>,
    pub zombie_state_layers: Vec<usize>,
    pub ground_swatch_layers: Vec<usize>,
    pub default_hidden_layers: Vec<usize>,
}

const SHADOW_POWER_STEMS: &[&str] = &[
    "dragonbabybruit",
    "dragonbruit",
    "dusklobber",
    "gloomvine",
    "grimrose",
    "guardshroom",
    "moonflower",
    "murkadamia",
    "nightshade",
    "noctarine",
    "powervine",
    "shadowpea",
    "shadowshroom",
];

pub fn special_layer_indices(pam: &PamInfo, source_name: &str) -> SpecialLayerIndices {
    let mut result = SpecialLayerIndices::default();
    let source_key = source_stem(source_name)
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    let nightshade = source_key.contains("nightshade");
    let shadow_power = SHADOW_POWER_STEMS
        .iter()
        .any(|stem| source_key.contains(stem));
    let names = pam
        .sprite
        .iter()
        .filter_map(|sprite| sprite.name.as_deref())
        .collect::<HashSet<_>>();

    for (index, sprite) in pam.sprite.iter().enumerate() {
        let Some(name) = sprite.name.as_deref() else {
            continue;
        };
        let lower = name.to_ascii_lowercase();
        if lower.starts_with("custom_") {
            result.plant_custom_layers.push(index);
        }
        if matches!(lower.as_str(), "ink" | "butter") {
            result.zombie_state_layers.push(index);
        }
        if matches!(lower.as_str(), "ground_swatch" | "ground_swatch_plane") {
            result.ground_swatch_layers.push(index);
        }
        if is_default_hidden(name, &lower, nightshade, shadow_power, &names) {
            result.default_hidden_layers.push(index);
        }
    }
    result
}

fn source_stem(source_name: &str) -> String {
    let file = source_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(source_name);
    let lower = file.to_ascii_lowercase();
    for extension in [
        ".pam.json",
        ".pam.yaml",
        ".pam.yml",
        ".pam.toml",
        ".json",
        ".yaml",
        ".yml",
        ".toml",
        ".pam",
        ".fla",
    ] {
        if lower.ends_with(extension) {
            return file[..file.len() - extension.len()].to_string();
        }
    }
    file.to_string()
}

fn is_default_hidden(
    name: &str,
    lower: &str,
    nightshade: bool,
    shadow_power: bool,
    names: &HashSet<&str>,
) -> bool {
    lower.starts_with("custom_")
        || lower.contains("armor")
        || lower == "magnet_item"
        || is_dark_layer(name, lower, shadow_power, names)
        || (nightshade && lower.ends_with("_pf"))
}

fn is_dark_layer(name: &str, lower: &str, shadow_power: bool, names: &HashSet<&str>) -> bool {
    if shadow_power && lower.contains("dark") {
        return true;
    }
    if let Some(base) = name.strip_suffix("_dark")
        && (names.contains(base)
            || names.contains(format!("{base}_normal").as_str())
            || names.contains(format!("{base}_white").as_str()))
    {
        return true;
    }
    if let Some((prefix, suffix)) = name.split_once("_dark_")
        && names.contains(format!("{prefix}_{suffix}").as_str())
    {
        return true;
    }
    if let Some(base) = name.strip_prefix("dark_")
        && names.contains(base)
    {
        return true;
    }
    false
}
