use std::sync::Mutex;

use log::{LevelFilter, Log, Metadata, Record};

const MAX_BUFFERED_LINES: usize = 5_000;
static BUFFER: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn push_line(line: String) {
    if let Ok(mut lines) = BUFFER.lock() {
        push_bounded_line(&mut lines, line);
    }
}

fn push_bounded_line(lines: &mut Vec<String>, line: String) {
    if lines.len() >= MAX_BUFFERED_LINES {
        let remove_count = lines.len() + 1 - MAX_BUFFERED_LINES;
        lines.drain(..remove_count);
    }
    lines.push(line);
}

struct BufferLogger {
    inner: Box<dyn Log>,
    level: LevelFilter,
}

impl Log for BufferLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.level || self.inner.enabled(metadata)
    }

    fn log(&self, record: &Record<'_>) {
        if record.level() <= self.level {
            push_line(format!(
                "[{}] {} - {}",
                record.level(),
                record.target(),
                record.args()
            ));
        }
        self.inner.log(record);
    }

    fn flush(&self) {
        self.inner.flush();
    }
}

pub fn init(inner: Box<dyn Log>, max_level: LevelFilter) {
    let logger = BufferLogger {
        inner,
        level: max_level,
    };
    log::set_max_level(max_level);
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::info!(target: "rton_editor::log", "Application log capture initialized");
    } else {
        push_line(
            "[ERROR] rton_editor::log - Failed to install application log capture".to_string(),
        );
    }
}

pub fn snapshot() -> String {
    BUFFER
        .lock()
        .map(|lines| lines.join("\n"))
        .unwrap_or_default()
}

pub fn clear() {
    if let Ok(mut lines) = BUFFER.lock() {
        lines.clear();
    }
}

#[cfg(target_arch = "wasm32")]
struct WebConsoleLogger;

#[cfg(target_arch = "wasm32")]
impl Log for WebConsoleLogger {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }

    fn log(&self, record: &Record<'_>) {
        let message = format!(
            "[{}] {} - {}",
            record.level(),
            record.target(),
            record.args()
        );
        match record.level() {
            log::Level::Error => web_sys::console::error_1(&message.into()),
            log::Level::Warn => web_sys::console::warn_1(&message.into()),
            _ => web_sys::console::log_1(&message.into()),
        }
    }

    fn flush(&self) {}
}

#[cfg(target_arch = "wasm32")]
pub fn init_wasm(max_level: LevelFilter) {
    init(Box::new(WebConsoleLogger), max_level);
}

#[cfg(test)]
mod tests {
    use super::{MAX_BUFFERED_LINES, push_bounded_line};

    #[test]
    fn bounded_buffer_keeps_the_newest_lines() {
        let mut lines = Vec::new();
        for index in 0..MAX_BUFFERED_LINES + 3 {
            push_bounded_line(&mut lines, format!("line-{index}"));
        }

        assert_eq!(lines.len(), MAX_BUFFERED_LINES);
        assert_eq!(lines.first().map(String::as_str), Some("line-3"));
        assert_eq!(
            lines.last().map(String::as_str),
            Some(format!("line-{}", MAX_BUFFERED_LINES + 2).as_str())
        );
    }
}
