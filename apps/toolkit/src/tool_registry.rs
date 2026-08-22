use crate::navigation::AppRoute;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ToolDescriptor {
    pub route: AppRoute,
    pub slug: &'static str,
    pub label: &'static str,
    pub glyph: &'static str,
    pub nav_glyph: &'static str,
    pub description: &'static str,
    pub tags: [&'static str; 3],
    pub action: &'static str,
    pub experimental: bool,
}

pub(crate) const TOOLS: [ToolDescriptor; 13] = [
    ToolDescriptor {
        route: AppRoute::Rsb,
        slug: "rsb",
        label: "RSB Archive",
        glyph: "▤",
        nav_glyph: "▤",
        description: "像桌面压缩软件一样浏览和编辑 RSB/RSG，直接预览 PTX，并按需提取资源。",
        tags: ["Archive", "zlib", "PTX"],
        action: "打开归档工具",
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::RsbPatch,
        slug: "rsb-patch",
        label: "RSB Patch",
        glyph: "⇄",
        nav_glyph: "⇄",
        description: "创建、检查并应用规范化 RSBP/VCDIFF 补丁，支持 stored 与 raw packet 模式。",
        tags: ["RSBP", "VCDIFF", "MD5"],
        action: "打开补丁工具",
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::Pak,
        slug: "pak",
        label: "PAK Archive",
        glyph: "PAK",
        nav_glyph: "▤",
        description: "像桌面压缩软件一样浏览和编辑 PopCap PAK，支持 PC、Xbox 360 与 TV ZIP 容器。",
        tags: ["Archive", "XOR", "ZIP"],
        action: "打开归档工具",
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::Dzip,
        slug: "dzip",
        label: "DZip Archive",
        glyph: "DZ",
        nav_glyph: "▤",
        description: "浏览、编辑和创建 DZip 与分卷归档，并为每个文件选择 DZ、Zlib、BZip 或 LZMA 压缩。",
        tags: ["Archive", "DZ", "Volumes"],
        action: "打开归档工具",
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::Smf,
        slug: "smf",
        label: "SMF Container",
        glyph: "SMF",
        nav_glyph: "◇",
        description: "自动识别并解开 PopCap SMF，或用可配置的 zlib 与 8/16 字节头封装任意文件。",
        tags: ["zlib", "32/64-bit", "MD5 Tag"],
        action: "打开压缩工具",
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::CryptData,
        slug: "crypt-data",
        label: "Crypt-Data",
        glyph: "◈",
        nav_glyph: "◈",
        description: "自动识别 CRYPT_RES，使用可配置密钥与范围加密或解密任意 PopCap 资源文件。",
        tags: ["CRYPT_RES", "XOR", "Local"],
        action: "打开加密工具",
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::Rton,
        slug: "rton",
        label: "RTON Editor",
        glyph: "{ }",
        nav_glyph: "{ }",
        description: "打开、搜索和编辑标准或紧凑 RTON，并在结构化文本格式之间转换。",
        tags: ["Tree", "Text", "Hex"],
        action: "打开编辑器",
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::Pam,
        slug: "pam",
        label: "PAM Viewer",
        glyph: "▶",
        nav_glyph: "▶",
        description: "预览动画、检查图片与精灵，并导出二进制、文本、图片或 FLA/XFL。",
        tags: ["WGPU Preview", "Timeline", "Export"],
        action: "打开工作区",
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::Particle,
        slug: "particle",
        label: "Particle Editor",
        glyph: "✦",
        nav_glyph: "✦",
        description: "查看并编辑 Particle/Trail 发射器、字段和轨道，在 XML 与多平台 compiled 之间转换。",
        tags: ["Emitters", "Curves", "XML"],
        action: "打开粒子编辑器",
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::Reanim,
        slug: "reanim",
        label: "REANIM Editor",
        glyph: "◫",
        nav_glyph: "◫",
        description: "编辑 REANIM 轨道与逐帧变换，在结构化文本、XFL 和多平台 compiled 之间转换。",
        tags: ["Timeline", "Transform", "XFL"],
        action: "打开动画编辑器",
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::Wem,
        slug: "wem",
        label: "WEM Audio",
        glyph: "♫",
        nav_glyph: "♫",
        description: "播放 WEM 与常见音频，在 Wwise WEM、WAV、Ogg Vorbis 和 AAC 之间转换。",
        tags: ["Playback", "Vorbis", "WAV"],
        action: "打开音频工具",
        experimental: false,
    },
    ToolDescriptor {
        route: AppRoute::Bnk,
        slug: "bnk",
        label: "BNK Archive",
        glyph: "BNK",
        nav_glyph: "▤",
        description: "浏览并回写 Wwise SoundBank 的 BKHD、HIRC 对象和内嵌 WEM，再校验并重建 BNK。",
        tags: ["SoundBank", "HIRC", "WEM"],
        action: "打开实验性工具",
        experimental: true,
    },
    ToolDescriptor {
        route: AppRoute::Newton,
        slug: "newton",
        label: "NEWTON Manifest",
        glyph: "N",
        nav_glyph: "◇",
        description: "查看和编辑 PvZ2 资源清单，并在 NEWTON、JSON、YAML 与 TOML 之间转换。",
        tags: ["Manifest", "JSON", "YAML"],
        action: "打开清单编辑器",
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
