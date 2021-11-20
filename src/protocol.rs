use serde::{Deserialize, Serialize};

use crate::users::UserId;

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKey {
    uid: UserId,
}

impl ApiKey {
    pub fn new(uid: UserId) -> Self {
        Self { uid }
    }

    pub fn uid(&self) -> UserId {
        self.uid
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginInfo {
    pub username: String,
    pub password: String,
}

pub trait IntoSerialize<T> {
    fn into_serialize(self) -> Result<T, String>;
}

impl<T> IntoSerialize<T> for anyhow::Result<T> {
    fn into_serialize(self) -> Result<T, String> {
        self.map_err(|err| format!("{:#}", err))
    }
}

pub trait EDeserialize<T> {
    fn deserialize(self) -> anyhow::Result<T>;
}

impl<T> EDeserialize<T> for Result<T, String> {
    fn deserialize(self) -> anyhow::Result<T> {
        self.map_err(|err| anyhow::anyhow!("{}", err))
    }
}

pub type ResResult<T> = Result<T, String>;
