pub mod fs;
pub mod protocol;
pub mod users;

use std::path::PathBuf;

use fs::{Fs, NodeEntry};
use serde::{Deserialize, Serialize};
use users::{Perms, UserDb, UserId, Username, ADMIN_ID};

#[derive(Debug, Serialize, Deserialize)]
pub enum Action {
    Read(PathBuf),
    Write(PathBuf, String),
    Rm(PathBuf),
    NewFile(PathBuf),
    NewDir(PathBuf),
    Exec(PathBuf),
    SetPerms(PathBuf, Perms),
    Ls(PathBuf),
    AddUser(Username),
    ChangePassword { old: String, new: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ActionRes {
    Ok,
    Read(String),
    Ls(Vec<NodeEntry>),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct System {
    fs: Fs,
    users: UserDb,
}

impl System {
    pub fn new() -> anyhow::Result<Self> {
        let fs = Fs::new();
        let users = UserDb::new()?;
        Ok(Self { fs, users })
    }

    pub fn login(&self, name: &str, pass: &str) -> anyhow::Result<UserId> {
        self.users.login(name, pass)
    }

    pub fn add_user(&mut self, uid: UserId, name: &str) -> anyhow::Result<UserId> {
        if uid == ADMIN_ID {
            self.users.add_user(name, "")
        } else {
            anyhow::bail!("only admin can manage users")
        }
    }

    pub fn exec(&mut self, uid: UserId, cmd: &Action) -> anyhow::Result<ActionRes> {
        let ok = |_| ActionRes::Ok;

        match cmd {
            Action::Read(path) => self
                .fs
                .read(uid, path)
                .map(|data| ActionRes::Read(data.into())),
            Action::Write(path, data) => self.fs.write(uid, path, data).map(ok),
            Action::Rm(path) => self.fs.rm(uid, path).map(ok),
            Action::NewFile(path) => self.fs.new_file(uid, path).map(ok),
            Action::NewDir(path) => self.fs.new_dir(uid, path).map(ok),
            Action::Exec(path) => self.fs.exec(uid, path).map(ok),
            Action::SetPerms(path, perms) => self.fs.set_perms(uid, path, *perms).map(ok),
            Action::Ls(path) => self.fs.ls(uid, path).map(ActionRes::Ls),
            Action::AddUser(name) => self.add_user(uid, name).map(|_| ActionRes::Ok),
            Action::ChangePassword { old, new } => self.users.change_pass(uid, old, new).map(ok),
        }
    }
}
