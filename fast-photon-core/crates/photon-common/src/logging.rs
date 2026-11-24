use serde::{Deserialize, Serialize};
use std::sync::Once;

static INIT_TRACING: Once = Once::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LogGroup {
    Camera,
    General,
    WebServer,
    Data,
    Config,
    VisionModule,
    NetworkTables,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LogLevel {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
    Trace = 4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogMessage {
    pub group: LogGroup,
    pub level: LogLevel,
    pub source: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct Logger {
    class_name: String,
    suffix: Option<String>,
    group: LogGroup,
}

impl Logger {
    pub fn init_global() {
        INIT_TRACING.call_once(|| {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
                )
                .try_init();
        });
    }

    pub fn new(class_name: impl Into<String>, group: LogGroup) -> Self {
        Self {
            class_name: class_name.into(),
            suffix: None,
            group,
        }
    }

    pub fn with_suffix(
        class_name: impl Into<String>,
        suffix: impl Into<String>,
        group: LogGroup,
    ) -> Self {
        Self {
            class_name: class_name.into(),
            suffix: Some(suffix.into()),
            group,
        }
    }

    pub fn prefix(&self) -> String {
        match &self.suffix {
            Some(s) => format!("[{:?} - {} ({})]", self.group, self.class_name, s),
            None => format!("[{:?} - {}]", self.group, self.class_name),
        }
    }

    pub fn error(&self, msg: impl AsRef<str>) {
        tracing::error!("{} {}", self.prefix(), msg.as_ref());
    }

    pub fn warn(&self, msg: impl AsRef<str>) {
        tracing::warn!("{} {}", self.prefix(), msg.as_ref());
    }

    pub fn info(&self, msg: impl AsRef<str>) {
        tracing::info!("{} {}", self.prefix(), msg.as_ref());
    }

    pub fn debug(&self, msg: impl AsRef<str>) {
        tracing::debug!("{} {}", self.prefix(), msg.as_ref());
    }

    pub fn trace(&self, msg: impl AsRef<str>) {
        tracing::trace!("{} {}", self.prefix(), msg.as_ref());
    }
}

pub struct KernelLogLogger;

impl KernelLogLogger {
    pub fn poll_recent_dmesg() -> Vec<String> {
        Vec::new()
    }
}
