#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tone {
    Info,
    Ok,
    Warn,
    Error,
}

impl Tone {
    pub(crate) fn class(self) -> &'static str {
        match self {
            Tone::Info => "info",
            Tone::Ok => "ok",
            Tone::Warn => "warn",
            Tone::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Status {
    pub(crate) message: String,
    pub(crate) tone: Tone,
}

impl Status {
    pub(crate) fn new(message: impl Into<String>, tone: Tone) -> Self {
        let message = message.into();
        match tone {
            Tone::Warn => log::warn!(target: "rton_editor::status", "{message}"),
            Tone::Error => log::error!(target: "rton_editor::status", "{message}"),
            Tone::Info | Tone::Ok => {}
        }
        Self { message, tone }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OpenTabError {
    Read(String),
    Decode(String),
}

impl OpenTabError {
    pub(crate) fn message(self) -> String {
        match self {
            Self::Read(message) | Self::Decode(message) => message,
        }
    }
}
