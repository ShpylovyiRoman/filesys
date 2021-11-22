use std::sync::{Mutex, MutexGuard};

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

#[derive(Debug, Serialize, Deserialize)]
pub struct Log {
    level: LogLevel,
    uid: UserId,
    msg: String,
    time: OffsetDateTime,
}

static FORMAT: Lazy<Vec<FormatItem>> = Lazy::new(|| {
    format_description::parse(
        "[month repr:short] [day] [hour]:[minute]:[second].[subsecond digits:3]",
    )
    .unwrap()
});

impl std::fmt::Display for Log {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let time = self.time.format(&FORMAT).map_err(|_| std::fmt::Error)?;

        write!(f, "{} {} {:?} {}", self.level, time, self.uid, self.msg)
    }
}

impl Log {
    pub fn new(level: LogLevel, uid: UserId, msg: String) -> Self {
        let time = OffsetDateTime::now_utc();
        Self {
            level,
            uid,
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

macro_rules! debug {
    ($uid: expr,  $($args : tt)*) => {
        let log = Log::new($crate::log::LogLevel::Debug, $uid, format!($($args)*));
        $crate::log::logger().log(log);
    };
}

macro_rules! info {
    ($uid: expr,  $($args : tt)*) => {
        let log = Log::new($crate::log::LogLevel::Info, $uid, format!($($args)*));
        $crate::log::logger().log(log);
    };
}

macro_rules! error {
    ($uid: expr,  $($args : tt)*) => {
        let log = Log::new($crate::log::LogLevel::Error, $uid, format!( $($args)*));
        $crate::log::logger().log(log);
    };
}
