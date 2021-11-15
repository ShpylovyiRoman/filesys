use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(u64);

pub const ADMIN_ID: UserId = UserId(0);

pub type Username = String;

pub enum Op {
    Read,
    Write,
    Exec,
}

impl<'a> From<&'a [Op]> for Perms {
    fn from(ops: &'a [Op]) -> Self {
        let mut this = Self::default();
        for op in ops {
            match op {
                Op::Read => this.read = true,
                Op::Write => this.write = true,
                Op::Exec => this.exec = true,
            }
        }
        this
    }
}

#[derive(Default)]
pub struct Perms {
    pub read: bool,
    pub write: bool,
    pub exec: bool,
}

pub struct User {
    id: UserId,
    name: Username,
    pass: String,
}

impl Perms {
    pub fn intersects(&self, ops: &[Op]) -> bool {
        ops.iter()
            .map(|op| match op {
                Op::Read => self.read,
                Op::Write => self.write,
                Op::Exec => self.exec,
            })
            .reduce(|a, b| a && b)
            .unwrap_or(false)
    }
}

#[derive(Default)]
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
}
