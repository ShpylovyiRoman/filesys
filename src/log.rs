use std::{
    fmt::Debug,
    sync::{Mutex, MutexGuard},
};

use once_cell::sync::Lazy;
use rocket::time::{
    format_description::{self, FormatItem},
    OffsetDateTime,
};
use serde::{Deserialize, Serialize};

use crate::users::UserId;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum LogLevel {
    Debug,
    Info,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let str = match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Error => "ERROR",
        };
        f.write_str(str)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Log {
    level: LogLevel,
    uid: Option<UserId>,
    msg: String,
    time: OffsetDateTime,
}

static FORMAT: Lazy<Vec<FormatItem>> = Lazy::new(|| {
    format_description::parse("[month repr:short] [day] [hour]:[minute]:[second]").unwrap()
});

impl std::fmt::Display for Log {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let time = self.time.format(&FORMAT).map_err(|_| std::fmt::Error)?;
        write!(f, "[{} {}] ", self.level, time)?;
        if let Some(uid) = self.uid {
            write!(f, "{:?} ", uid)?;
        }
        write!(f, "{}", self.msg)
    }
}

impl Log {
    pub fn new_with_uid(level: LogLevel, uid: UserId, msg: String) -> Self {
        let time = OffsetDateTime::now_utc();
        Self {
            level,
            uid: Some(uid),
            msg,
            time,
        }
    }

    pub fn new(level: LogLevel, msg: String) -> Self {
        let time = OffsetDateTime::now_utc();
        Self {
            level,
            uid: None,
            msg,
            time,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Logger {
    logs: Vec<Log>,
}

impl Logger {
    pub fn log(&mut self, log: Log) {
        self.logs.push(log);
    }

    /// Get a reference to the logger's logs.
    pub fn logs(&self) -> &[Log] {
        self.logs.as_ref()
    }
}

static LOGGER: Lazy<Mutex<Logger>> = Lazy::new(Mutex::default);

pub fn set_logger(log: Logger) -> Logger {
    let mut entry = logger();

    std::mem::replace(&mut *entry, log)
}

pub fn take_logger() -> Logger {
    set_logger(Default::default())
}

pub fn logger() -> MutexGuard<'static, Logger> {
    LOGGER.lock().expect("acquiring mutex")
}

#[macro_export]
macro_rules! debug {
    ($uid: expr =>  $($args : tt)*) => {{
        use $crate::log::*;
        let log = Log::new_with_uid(LogLevel::Debug, $uid, format!($($args)*));
        logger().log(log);
    }};
    ($($args : tt)*) => {{
        use $crate::log::*;
        let log = Log::new(LogLevel::Debug, format!($($args)*));
        logger().log(log);
    }};
}

#[macro_export]
macro_rules! info {
    ($uid: expr =>  $($args : tt)*) => {{
        use $crate::log::*;
        let log = Log::new_with_uid(LogLevel::Info, $uid, format!($($args)*));
        logger().log(log);
    }};
    ($($args : tt)*) => {{
        use $crate::log::*;
        let log = Log::new(LogLevel::Info, format!($($args)*));
        logger().log(log);
    }};
}

#[macro_export]
macro_rules! error {
    ($uid: expr => $($args : tt)*) => {{
        use $crate::log::*;
        let log = Log::new_with_uid(LogLevel::Error, $uid, format!($($args)*));
        logger().log(log);
    }};
    ($($args : tt)*) => {{
        use $crate::log::*;
        let log = Log::new(LogLevel::Error, format!($($args)*));
        logger().log(log);
    }};
}
