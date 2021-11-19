use std::{collections::HashMap, str::FromStr};

use anyhow::anyhow;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UserId(u64);

impl Default for UserId {
    fn default() -> Self {
        ADMIN_ID
    }
}

impl UserId {
    pub fn new(num: u64) -> Self {
        Self(num)
    }

    pub fn tick_next(&mut self) -> Self {
        let this = *self;
        self.0 += 1;
        this
    }

    pub fn get_next(&self) -> Self {
        Self(self.0 + 1)
    }
}

pub const ADMIN_ID: UserId = UserId(0);

pub type Username = String;

#[derive(Debug, Serialize, Deserialize)]
pub enum Op {
    Read,
    Write,
    Exec,
    Control,
}

impl<'a> From<&'a [Op]> for Perms {
    fn from(ops: &'a [Op]) -> Self {
        let mut this = Self::default();
        for op in ops {
            match op {
                Op::Read => this.read = true,
                Op::Write => this.write = true,
                Op::Exec => this.exec = true,
                Op::Control => this.control = true,
            }
        }
        this
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct Perms {
    pub read: bool,
    pub write: bool,
    pub exec: bool,
    pub control: bool,
}

impl FromStr for Perms {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut this = Self::default();
        for c in s.chars() {
            match c {
                'r' => this.read = true,
                'w' => this.write = true,
                'e' => this.exec = true,
                'c' => this.control = true,
                _ => anyhow::bail!("unexpected character: {:?}", c),
            }
        }
        Ok(this)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    id: UserId,
    name: Username,
    pass: String,
}

impl User {
    pub fn new(id: UserId, name: Username, pass: &str) -> anyhow::Result<Self> {
        let mut this = Self {
            id,
            name,
            pass: Default::default(),
        };
        this.change_pass(pass)?;
        Ok(this)
    }

    pub fn verify_pass(&self, pass: &str) -> anyhow::Result<Option<()>> {
        let parsed_hash =
            PasswordHash::new(&self.pass).map_err(|err| anyhow!("parsing hash: {}", err))?;
        let argon2 = Argon2::default();
        Ok(argon2.verify_password(pass.as_bytes(), &parsed_hash).ok())
    }

    pub fn change_pass(&mut self, pass: &str) -> anyhow::Result<()> {
        let salt = SaltString::generate(rand::thread_rng());

        let argon2 = Argon2::default();

        let pass = argon2
            .hash_password(pass.as_bytes(), &salt)
            .map_err(|err| anyhow!("deriving hash: {}", err))?
            .to_string();

        self.pass = pass;
        Ok(())
    }
}

impl Perms {
    pub fn intersects(&self, ops: &[Op]) -> bool {
        ops.iter()
            .map(|op| match op {
                Op::Read => self.read,
                Op::Write => self.write,
                Op::Exec => self.exec,
                Op::Control => self.control,
            })
            .reduce(|a, b| a && b)
            .unwrap_or(false)
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct AccessMap {
    perms: HashMap<UserId, Perms>,
}

impl AccessMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allows(&self, uid: UserId, ops: &[Op]) -> bool {
        if uid == ADMIN_ID {
            true
        } else if let Some(perms) = self.perms.get(&uid) {
            perms.intersects(ops)
        } else {
            false
        }
    }

    pub fn set(&mut self, uid: UserId, perms: impl Into<Perms>) {
        if uid != ADMIN_ID {
            self.perms.insert(uid, perms.into());
        }
    }

    pub fn get(&self, uid: UserId) -> Perms {
        self.perms.get(&uid).copied().unwrap_or_default()
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct UserDb {
    id_ctr: UserId,
    unames: HashMap<Username, UserId>,
    users: HashMap<UserId, User>,
}

fn wrong_uname() -> anyhow::Error {
    anyhow!("wrong username or password")
}

impl UserDb {
    pub fn new() -> anyhow::Result<Self> {
        let mut this = Self::default();
        this.add_user("admin", "")?;
        Ok(this)
    }

    pub fn add_user(&mut self, name: &str, pass: &str) -> anyhow::Result<UserId> {
        if self.unames.contains_key(name) {
            anyhow::bail!("user exists")
        }
        let id = self.id_ctr.tick_next();

        let user = User::new(id, name.to_string(), pass)?;
        self.unames.insert(name.to_string(), id);
        self.users.insert(id, user);

        Ok(id)
    }

    pub fn login(&self, name: &str, pass: &str) -> anyhow::Result<UserId> {
        let uid = self.unames.get(name).ok_or_else(wrong_uname)?;
        self.login_with_id(*uid, pass).map(|_| *uid)
    }

    pub fn login_with_id(&self, uid: UserId, pass: &str) -> anyhow::Result<()> {
        let user = self.users.get(&uid).ok_or_else(wrong_uname)?;
        user.verify_pass(pass)?.ok_or_else(wrong_uname)
    }

    pub fn change_pass(
        &mut self,
        uid: UserId,
        old_pass: &str,
        new_pass: &str,
    ) -> anyhow::Result<()> {
        self.login_with_id(uid, old_pass)?;
        let user = self
            .users
            .get_mut(&uid)
            .expect("already checked for existence");

        user.change_pass(new_pass)
    }
}
