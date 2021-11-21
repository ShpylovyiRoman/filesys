use std::{collections::HashMap, str::FromStr};

use anyhow::anyhow;
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use serde::{Deserialize, Serialize};

const MAX_LOGIN_TRIES: usize = 3;

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

impl std::fmt::Display for Perms {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let read = if self.read { 'r' } else { '-' };
        let write = if self.write { 'w' } else { '-' };
        let exec = if self.exec { 'x' } else { '-' };
        let control = if self.control { 'c' } else { '-' };
        write!(f, "{}{}{}{}", read, write, exec, control)
    }
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
    last_login_tries: usize,
}

impl User {
    pub fn new(id: UserId, name: Username, pass: &str) -> anyhow::Result<Self> {
        let mut this = Self {
            id,
            name,
            pass: Default::default(),
            last_login_tries: 0,
        };
        this.change_pass(pass)?;
        Ok(this)
    }

    fn is_blocked(&self) -> bool {
        self.last_login_tries >= MAX_LOGIN_TRIES
    }

    pub fn verify_pass(&mut self, pass: &str) -> anyhow::Result<()> {
        if self.is_blocked() {
            anyhow::bail!("account if blocked")
        }

        let parsed_hash =
            PasswordHash::new(&self.pass).map_err(|err| anyhow!("parsing hash: {}", err))?;
        let argon2 = Argon2::default();
        let ok = argon2
            .verify_password(pass.as_bytes(), &parsed_hash)
            .is_ok();

        if ok {
            self.reset_login_tries();
            Ok(())
        } else {
            self.inc_login_tries()?;
            Err(wrong_uname())
        }
    }

    fn inc_login_tries(&mut self) -> anyhow::Result<()> {
        self.last_login_tries += 1;
        if self.last_login_tries >= MAX_LOGIN_TRIES {
            anyhow::bail!("account is blocked")
        } else {
            Ok(())
        }
    }

    fn reset_login_tries(&mut self) {
        self.last_login_tries = 0;
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

    pub fn login(&mut self, name: &str, pass: &str) -> anyhow::Result<UserId> {
        let uid = *self.unames.get(name).ok_or_else(wrong_uname)?;
        self.login_with_id(uid, pass).map(|_| uid)
    }

    pub fn login_with_id(&mut self, uid: UserId, pass: &str) -> anyhow::Result<()> {
        let user = self.users.get_mut(&uid).ok_or_else(wrong_uname)?;
        user.verify_pass(pass)
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

    pub fn id_of(&self, username: &str) -> anyhow::Result<UserId> {
        self.unames
            .get(username)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("user not found"))
    }

    pub fn unblock(&mut self, username: &str) -> anyhow::Result<()> {
        let id = self.id_of(username)?;
        let user = self.users.get_mut(&id).expect("should exists");
        user.reset_login_tries();
        Ok(())
    }
}
