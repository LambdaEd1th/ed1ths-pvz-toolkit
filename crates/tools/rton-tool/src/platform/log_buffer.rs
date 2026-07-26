use log::{LevelFilter, Log, Metadata, Record};

fn record_message(record: &Record<'_>) -> String {
    record.args().to_string()
}

fn record_scope(target: &str) -> String {
    target
        .rsplit("::")
        .next()
        .filter(|scope| !scope.is_empty())
        .unwrap_or("APP")
        .replace('_', "-")
        .to_ascii_uppercase()
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
            toolkit_ui::push_application_log(
                "RTON",
                record.level().as_str(),
                &record_scope(record.target()),
                record_message(record),
            );
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
        log::info!(
            target: "rton_editor::app",
            "Initialized (version {})",
            env!("CARGO_PKG_VERSION")
        );
    } else {
        toolkit_ui::push_application_log(
            "RTON",
            "ERROR",
            "APP",
            "Failed to initialize log capture",
        );
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
        let message = toolkit_ui::format_application_log_line(
            "RTON",
            record.level().as_str(),
            &record_scope(record.target()),
            &record_message(record),
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
    use super::record_scope;

    #[test]
    fn record_targets_become_shared_scope_names() {
        assert_eq!(record_scope("rton_editor::status"), "STATUS");
        assert_eq!(record_scope("rton_editor::preferences"), "PREFERENCES");
        assert_eq!(record_scope(""), "APP");
    }
}
