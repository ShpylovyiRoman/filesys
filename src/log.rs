use std::sync::{Mutex, MutexGuard};

use once_cell::sync::Lazy;
use rocket::time::OffsetDateTime;

use crate::users::UserId;

#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    Debug,
    Info,
    Error,
}

#[derive(Debug)]
pub struct Log {
    level: LogLevel,
    uid: UserId,
    msg: String,
    time: OffsetDateTime,
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

#[derive(Debug, Default)]
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
