use dioxus::prelude::*;

const REPOSITORY_URL: &str = "https://github.com/LambdaEd1th/ed1ths-pvz-toolkit";
const TECH_STACK: [&str; 6] = [
    "Rust",
    "Dioxus 0.7",
    "WGPU 30",
    "WebAssembly",
    "Serde",
    "WebGPU",
];

#[component]
pub(crate) fn AboutPage() -> Element {
    rsx! {
        div { class: "tk-about-scroll",
            div { class: "tk-about-page",
                header { class: "tk-about-hero",
                    span { class: "tk-kicker", "About" }
                    h1 { "关于 Ed1th's PvZ Toolkit" }
                    p {
                        "面向 PopCap / PvZ 资源格式的开源工具工作区。"
                        "归档浏览、RSBP 补丁、SMF 压缩、Crypt-Data 加解密、RTON 数据、Particle 与 REANIM 编辑、PAM 动画查看、WEM 音频转换、NEWTON 清单维护与可复用格式库在这里共享一致的体验。"
                    }
                }

                div { class: "tk-about-grid",
                    article { class: "tk-about-card tk-about-card--mission tk-about-span-2",
                        div { class: "tk-about-card-heading",
                            div {
                                h2 { "开放、专注、本地优先" }
                                span { "Built for format tooling" }
                            }
                            span { class: "tk-about-version", "v{env!(\"CARGO_PKG_VERSION\")} Preview" }
                        }
                        div { class: "tk-about-divider" }
                        p {
                            "Toolkit 将多个专业工具组合到一个应用外壳中，同时让 "
                            code { "rsb-archive" }
                            "、"
                            code { "rsb-patch" }
                            "、"
                            code { "pak-archive" }
                            "、"
                            code { "compiled-text" }
                            "、"
                            code { "crypt-data" }
                            " 与 "
                            code { "serde-rton" }
                            "、"
                            code { "pam-codec" }
                            "、"
                            code { "particle-codec" }
                            "、"
                            code { "reanim-codec" }
                            "、"
                            code { "wem-audio" }
                            "、"
                            code { "newton-manifest" }
                            " 保持独立、可测试并可被其他 Rust 项目直接使用。"
                        }
                        p {
                            "文件处理在本地完成；Web 与桌面版本共用相同的工作流，"
                            "不依赖 CLI，也不要求聚合 SDK。"
                        }
                        div { class: "tk-about-principles",
                            span { "Local processing" }
                            span { "Independent libraries" }
                            span { "One workspace" }
                        }
                    }

                    article { class: "tk-about-card tk-about-card--maintainer",
                        span { class: "tk-about-card-label", "Maintainer" }
                        div { class: "tk-about-maintainer-mark", aria_hidden: "true", "E" }
                        h2 { "LambdaEd1th" }
                        p { "Independent developer" }
                        small { "Formats first · UI together" }
                    }

                    article { class: "tk-about-card tk-about-card--stack",
                        span { class: "tk-about-card-label", "Tech stack" }
                        h2 { "Rust-native foundation" }
                        div { class: "tk-about-tech-list",
                            for technology in TECH_STACK {
                                span { "{technology}" }
                            }
                        }
                        p {
                            "共享 Dioxus UI 外壳，并由 WGPU / WebGPU 提供 PAM 实时渲染。"
                        }
                    }

                    article { class: "tk-about-card tk-about-card--credits tk-about-span-2",
                        h2 { "Credits & Copyright" }
                        div { class: "tk-about-credit-grid",
                            section {
                                h3 { "Format research" }
                                ul {
                                    li {
                                        span { aria_hidden: "true" }
                                        p {
                                            strong { "PopCap / Plants vs. Zombies" }
                                            "文件格式与兼容性研究对象"
                                        }
                                    }
                                }
                            }
                            section {
                                h3 { "License & open source" }
                                p {
                                    "本项目源代码遵循 "
                                    strong { "AGPL-3.0-or-later" }
                                    "。独立格式库可直接从工作区引用。"
                                }
                                a {
                                    class: "tk-about-repository",
                                    href: REPOSITORY_URL,
                                    target: "_blank",
                                    rel: "noreferrer",
                                    span { "GitHub repository" }
                                    b { aria_hidden: "true", "↗" }
                                }
                            }
                        }
                    }

                    article { class: "tk-about-card tk-about-card--formats tk-about-span-3",
                        div { class: "tk-about-card-heading",
                            div {
                                span { class: "tk-about-card-label", "Workspaces & libraries" }
                                h2 { "专业工具，共享设计语言" }
                            }
                            span { class: "tk-about-count", "13 tools" }
                        }
                        div { class: "tk-about-format-grid",
                            section { class: "tk-about-format tk-about-format--rsb",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "RSB" }
                                    div {
                                        h3 { "RSB Archive" }
                                        code { "rsb-archive" }
                                    }
                                }
                                p { "归档与 RSG 数据包浏览、zlib 验证、资源提取，以及 PTX 纹理编解码与预览。" }
                            }
                            section { class: "tk-about-format tk-about-format--rsb-patch",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "⇄" }
                                    div {
                                        h3 { "RSB Patch" }
                                        code { "rsb-patch" }
                                    }
                                }
                                p { "规范化 RSBP 容器与 VCDIFF 差分，支持检查、创建和应用补丁。" }
                            }
                            section { class: "tk-about-format tk-about-format--pak",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "PAK" }
                                    div {
                                        h3 { "PAK Archive" }
                                        code { "pak-archive" }
                                    }
                                }
                                p { "PC XOR、Plain、Xbox 360 与 TV ZIP 容器的浏览、编辑、提取和规范化回写。" }
                            }
                            section { class: "tk-about-format tk-about-format--dzip",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "◇" }
                                    div {
                                        h3 { "DZip Archive" }
                                        code { "dzip-archive" }
                                    }
                                }
                                p { "DZip 归档与分卷浏览、编辑、提取，以及 DZ、zlib、BZip2 和 LZMA 编解码。" }
                            }
                            section { class: "tk-about-format tk-about-format--smf",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "◇" }
                                    div {
                                        h3 { "SMF Container" }
                                        code { "smf-container" }
                                    }
                                }
                                p { "PopCap SMF 自动识别、流式解压与可配置 zlib 封装。" }
                            }
                            section { class: "tk-about-format tk-about-format--crypt-data",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "◈" }
                                    div {
                                        h3 { "Crypt-Data" }
                                        code { "crypt-data" }
                                    }
                                }
                                p { "CRYPT_RES 自动识别、可配置前缀 XOR、原始长度校验与本地加解密转换。" }
                            }
                            section { class: "tk-about-format tk-about-format--rton",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "RTON" }
                                    div {
                                        h3 { "RTON Editor" }
                                        code { "serde-rton" }
                                    }
                                }
                                p { "树、文本与十六进制编辑，标准/紧凑 RTON 转换及可选加密支持。" }
                            }
                            section { class: "tk-about-format tk-about-format--pam",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "PAM" }
                                    div {
                                        h3 { "PAM Viewer" }
                                        code { "pam-codec" }
                                    }
                                }
                                p { "动画预览、资源检查、时间轴控制，以及开放格式与 FLA/XFL 导出。" }
                            }
                            section { class: "tk-about-format tk-about-format--particle",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "✦" }
                                    div {
                                        h3 { "Particle Editor" }
                                        code { "particle-codec" }
                                    }
                                }
                                p { "Particle 与 Trail 的发射器、字段和轨道检查，以及 XML 与多平台 compiled 双向转换。" }
                            }
                            section { class: "tk-about-format tk-about-format--reanim",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "◫" }
                                    div {
                                        h3 { "REANIM Editor" }
                                        code { "reanim-codec" }
                                    }
                                }
                                p { "REANIM 轨道与逐帧变换编辑，以及结构化文本、XFL 与多平台 compiled 转换。" }
                            }
                            section { class: "tk-about-format tk-about-format--wem",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "WEM" }
                                    div {
                                        h3 { "WEM Audio" }
                                        code { "wem-audio" }
                                    }
                                }
                                p { "WEM 与常见音频的本地播放、编码、解码及无损音频流重封装。" }
                            }
                            section { class: "tk-about-format tk-about-format--bnk",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "BNK" }
                                    div {
                                        h3 { "BNK Archive" }
                                        code { "bnk-archive" }
                                    }
                                }
                                p { "实验性 Wwise SoundBank 浏览、HIRC 结构编辑、内嵌 WEM 提取与重建回写。" }
                            }
                            section { class: "tk-about-format tk-about-format--newton",
                                div { class: "tk-about-format-heading",
                                    span { aria_hidden: "true", "N" }
                                    div {
                                        h3 { "NEWTON Manifest" }
                                        code { "newton-manifest" }
                                    }
                                }
                                p { "资源清单、资源组、槽位、子组引用与图集几何的查看、校验和编辑。" }
                            }
                        }
                    }

                    article { class: "tk-about-card tk-about-card--notice tk-about-span-3",
                        div {
                            span { class: "tk-about-card-label", "Unofficial project" }
                            h2 { "版权与项目声明" }
                        }
                        p {
                            "本项目与 Electronic Arts、PopCap Games 或 Plants vs. Zombies 官方无隶属关系。"
                            "游戏名称、素材及相关商标归其各自权利方所有。"
                            "仓库中的兼容性样例仅用于格式研究与测试；使用者应自行确保资源使用获得适当授权。"
                        }
                    }
                }

                footer { class: "tk-about-footer",
                    span { "Ed1th's PvZ Toolkit" }
                    p { "Open formats · Local tools · Shared workspace" }
                    b { "2026" }
                }
            }
        }
    }
}
