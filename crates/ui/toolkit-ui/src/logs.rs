use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, OnceLock};
#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_APPLICATION_LOG_LINES: usize = 5_000;
static APPLICATION_LOGS: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();

fn lines() -> MutexGuard<'static, VecDeque<String>> {
    APPLICATION_LOGS
        .get_or_init(|| Mutex::new(VecDeque::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn push_bounded_line(lines: &mut VecDeque<String>, line: String) {
    lines.push_back(line);
    while lines.len() > MAX_APPLICATION_LOG_LINES {
        lines.pop_front();
    }
}

fn timestamp() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    #[cfg(target_arch = "wasm32")]
    let seconds = (js_sys::Date::now() / 1_000.0) as u64;
    let seconds = seconds % 86_400;
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3_600,
        (seconds / 60) % 60,
        seconds % 60
    )
}

pub fn format_application_log_line(
    source: &str,
    level: &str,
    scope: &str,
    message: &str,
) -> String {
    format!(
        "[{}] [{}] [{}] [{}] {message}",
        timestamp(),
        source.to_ascii_uppercase(),
        level.to_ascii_uppercase(),
        scope.to_ascii_uppercase()
    )
}

pub fn push_application_log(source: &str, level: &str, scope: &str, message: impl AsRef<str>) {
    let message = message.as_ref();
    if message.is_empty() {
        return;
    }

    let mut lines = lines();
    for message_line in message.lines() {
        push_bounded_line(
            &mut lines,
            format_application_log_line(source, level, scope, message_line),
        );
    }
}

pub fn application_log_lines() -> Vec<String> {
    lines().iter().cloned().collect()
}

pub fn clear_application_logs() {
    lines().clear();
}

#[cfg(test)]
mod tests {
    use super::{MAX_APPLICATION_LOG_LINES, format_application_log_line, push_bounded_line};
    use std::collections::VecDeque;

    #[test]
    fn application_log_capacity_keeps_the_newest_lines() {
        let mut lines = VecDeque::new();
        for index in 0..MAX_APPLICATION_LOG_LINES + 2 {
            push_bounded_line(&mut lines, index.to_string());
        }

        assert_eq!(lines.len(), MAX_APPLICATION_LOG_LINES);
        assert_eq!(lines.front().map(String::as_str), Some("2"));
    }

    #[test]
    fn application_log_line_has_the_shared_format() {
        let line = format_application_log_line("pam", "info", "renderer", "Initialized (worker)");

        assert_eq!(&line[0..1], "[");
        assert_eq!(&line[3..4], ":");
        assert_eq!(&line[6..7], ":");
        assert_eq!(&line[10..], " [PAM] [INFO] [RENDERER] Initialized (worker)");
    }
}
