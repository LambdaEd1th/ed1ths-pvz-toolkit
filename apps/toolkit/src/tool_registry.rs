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

pub(crate) const TOOLS: [ToolDescriptor; 2] = [
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
        route: AppRoute::Rton,
        slug: "rton",
        label: "RTON Editor",
        glyph: "{ }",
        description: "打开、搜索和编辑标准或紧凑 RTON，并在结构化文本格式之间转换。",
        tags: ["Tree", "Text", "Hex"],
        action: "打开编辑器",
    },
];
