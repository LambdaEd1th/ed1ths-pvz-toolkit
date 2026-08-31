use dioxus::prelude::*;

const REPORT_RUNTIME_READY: &str = "if (window.toolkitStartup) window.toolkitStartup.update(36);";
const REPORT_APPEARANCE_READY: &str =
    "if (window.toolkitStartup) window.toolkitStartup.update(62);";
const REPORT_SHELL_READY: &str = "if (window.toolkitStartup) window.toolkitStartup.update(88);";
const FINISH_STARTUP: &str = "if (window.toolkitStartup) window.toolkitStartup.finish();";

pub(crate) fn report_runtime_ready() {
    let _ = document::eval(REPORT_RUNTIME_READY);
}

pub(crate) fn report_appearance_ready() {
    let _ = document::eval(REPORT_APPEARANCE_READY);
}

pub(crate) fn report_shell_ready() {
    let _ = document::eval(REPORT_SHELL_READY);
}

pub(crate) fn finish() {
    let _ = document::eval(FINISH_STARTUP);
}
