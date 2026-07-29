use crate::navigation::AppRoute;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ToolDescriptor {
    pub route: AppRoute,
    pub slug: &'static str,
    pub label: &'static str,
    pub glyph: &'static str,
    pub description: &'static str,
    pub tags: [&'static str; 3],
    pub action: &'static str,
}

pub(crate) const TOOLS: [ToolDescriptor; 4] = [
    ToolDescriptor {
        route: AppRoute::Rsb,
        slug: "rsb",
        label: "RSB Archive",
        glyph: "▤",
        description: "像桌面压缩软件一样浏览和编辑 RSB/RSG，直接预览 PTX，并按需提取资源。",
        tags: ["Archive", "zlib", "PTX"],
        action: "打开归档工具",
    },
    ToolDescriptor {
        route: AppRoute::Rton,
        slug: "rton",
        label: "RTON Editor",
        glyph: "{ }",
        description: "打开、搜索和编辑标准或紧凑 RTON，并在结构化文本格式之间转换。",
        tags: ["Tree", "Text", "Hex"],
        action: "打开编辑器",
    },
    ToolDescriptor {
        route: AppRoute::Pam,
        slug: "pam",
        label: "PAM Viewer",
        glyph: "▶",
        description: "预览动画、检查图片与精灵，并导出二进制、文本、图片或 FLA/XFL。",
        tags: ["WGPU Preview", "Timeline", "Export"],
        action: "打开工作区",
    },
    ToolDescriptor {
        route: AppRoute::Wem,
        slug: "wem",
        label: "WEM Audio",
        glyph: "♫",
        description: "播放 WEM 与常见音频，在 Wwise WEM、WAV、Ogg Vorbis 和 AAC 之间转换。",
        tags: ["Playback", "Vorbis", "WAV"],
        action: "打开音频工具",
    },
];

#[cfg(test)]
mod tests {
    use super::TOOLS;
    use crate::navigation::AppRoute;

    #[test]
    fn tools_follow_archive_data_animation_audio_order() {
        assert_eq!(
            TOOLS.map(|tool| tool.route),
            [AppRoute::Rsb, AppRoute::Rton, AppRoute::Pam, AppRoute::Wem,]
        );
    }
}
