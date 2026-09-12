use crate::navigation::AppRoute;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ToolDescriptor {
    pub route: AppRoute,
    pub slug: &'static str,
    pub label: &'static str,
    pub glyph: &'static str,
    pub nav_glyph: &'static str,
    pub tags: [&'static str; 3],
    pub experimental: bool,
}

pub(crate) const TOOLS: [ToolDescriptor; 14] = [
    ToolDescriptor {
        route: AppRoute::Rsb,
        slug: "rsb",
        label: "RSB Archive",
        glyph: "▤",
        nav_glyph: "▤",
        tags: ["Archive", "zlib", "PTX"],
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::RsbPatch,
        slug: "rsb-patch",
        label: "RSB Patch",
        glyph: "⇄",
        nav_glyph: "⇄",
        tags: ["RSBP", "VCDIFF", "MD5"],
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::Pak,
        slug: "pak",
        label: "PAK Archive",
        glyph: "PAK",
        nav_glyph: "▤",
        tags: ["Archive", "XOR", "ZIP"],
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::Dzip,
        slug: "dzip",
        label: "DZip Archive",
        glyph: "DZ",
        nav_glyph: "▤",
        tags: ["Archive", "DZ", "Volumes"],
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::Smf,
        slug: "smf",
        label: "SMF Container",
        glyph: "SMF",
        nav_glyph: "◇",
        tags: ["zlib", "32/64-bit", "MD5 Tag"],
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::CompiledText,
        slug: "compiled-text",
        label: "Compiled Text",
        glyph: "≋",
        nav_glyph: "≋",
        tags: ["Rijndael", "Base64", "SMF"],
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::CryptData,
        slug: "crypt-data",
        label: "Crypt-Data",
        glyph: "◈",
        nav_glyph: "◈",
        tags: ["CRYPT_RES", "XOR", "Local"],
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::Rton,
        slug: "rton",
        label: "RTON Editor",
        glyph: "{ }",
        nav_glyph: "{ }",
        tags: ["Tree", "Text", "Hex"],
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::Pam,
        slug: "pam",
        label: "PAM Editor",
        glyph: "▶",
        nav_glyph: "▶",
        tags: ["WGPU Preview", "Timeline", "Export"],
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::Particle,
        slug: "particle",
        label: "Particle Editor",
        glyph: "✦",
        nav_glyph: "✦",
        tags: ["Emitters", "Curves", "XML"],
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::Reanim,
        slug: "reanim",
        label: "REANIM Editor",
        glyph: "◫",
        nav_glyph: "◫",
        tags: ["Timeline", "Transform", "XFL"],
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::Wem,
        slug: "wem",
        label: "WEM Audio",
        glyph: "♫",
        nav_glyph: "♫",
        tags: ["Playback", "Vorbis", "WAV"],
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::Bnk,
        slug: "bnk",
        label: "BNK Archive",
        glyph: "BNK",
        nav_glyph: "▤",
        tags: ["SoundBank", "HIRC", "WEM"],
        experimental: true,
    },
    ToolDescriptor {
        route: AppRoute::Newton,
        slug: "newton",
        label: "NEWTON Manifest",
        glyph: "N",
        nav_glyph: "◇",
        tags: ["Manifest", "JSON", "YAML"],
        experimental: false,
    },
];

#[cfg(test)]
mod tests {
    use super::TOOLS;
    use crate::navigation::AppRoute;

    #[test]
    fn tools_place_newton_after_audio() {
        assert_eq!(
            TOOLS.map(|tool| tool.route),
            [
                AppRoute::Rsb,
                AppRoute::RsbPatch,
                AppRoute::Pak,
                AppRoute::Dzip,
                AppRoute::Smf,
                AppRoute::CompiledText,
                AppRoute::CryptData,
                AppRoute::Rton,
                AppRoute::Pam,
                AppRoute::Particle,
                AppRoute::Reanim,
                AppRoute::Wem,
                AppRoute::Bnk,
                AppRoute::Newton,
            ]
        );
    }

    #[test]
    fn navigation_glyphs_are_symbols_instead_of_abbreviations() {
        assert!(TOOLS.iter().all(|tool| {
            !tool
                .nav_glyph
                .chars()
                .any(|character| character.is_ascii_alphanumeric())
        }));
    }
}
