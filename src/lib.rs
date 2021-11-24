pub mod fs;
pub mod log;
pub mod protocol;
pub mod users;

use std::path::PathBuf;

use fs::{Fs, NodeEntry};
use log::{Log, Logger};
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
    SetPerms(PathBuf, Username, Perms),
    Ls(PathBuf),
    AddUser(Username),
    ChangePassword { old: String, new: String },
    Unblock(Username),
    Logs,
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::Read(path) => write!(f, "read({:?})", path),
            Action::Write(path, _) => write!(f, "write({:?})", path),
            Action::Rm(path) => write!(f, "rm({:?})", path),
            Action::NewFile(path) => write!(f, "new-file({:?})", path),
            Action::NewDir(path) => write!(f, "new-dir({:?})", path),
            Action::Exec(path) => write!(f, "exec({:?})", path),
            Action::SetPerms(path, user, perms) => {
                write!(f, "set-perms({:?}, {:?}, {})", path, user, perms)
            }
            Action::Ls(path) => write!(f, "ls({:?})", path),
            Action::AddUser(user) => write!(f, "add-user({:?})", user),
            Action::ChangePassword { .. } => write!(f, "change-pass"),
            Action::Unblock(user) => write!(f, "unblock({:?})", user),
            Action::Logs => write!(f, "logs"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ActionRes {
    Ok,
    Read(String),
    Ls(Vec<NodeEntry>),
    Logs(Vec<Log>),
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

    pub fn login(&mut self, name: &str, pass: &str) -> anyhow::Result<UserId> {
        info!("new login with {:?}", name);
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

        let res = match cmd {
            Action::Read(path) => self
                .fs
                .read(uid, path)
                .map(|data| ActionRes::Read(data.into())),
            Action::Write(path, data) => self.fs.write(uid, path, data).map(ok),
            Action::Rm(path) => self.fs.rm(uid, path).map(ok),
            Action::NewFile(path) => self.fs.new_file(uid, path).map(ok),
            Action::NewDir(path) => self.fs.new_dir(uid, path).map(ok),
            Action::Exec(path) => self.fs.exec(uid, path).map(ok),
            Action::SetPerms(path, username, perms) => {
                let for_user = self.users.id_of(username)?;
                self.fs.set_perms(uid, for_user, path, *perms).map(ok)
            }
            Action::Ls(path) => self.fs.ls(uid, path).map(ActionRes::Ls),
            Action::AddUser(name) => self.add_user(uid, name).map(|_| ActionRes::Ok),
            Action::ChangePassword { old, new } => self.users.change_pass(uid, old, new).map(ok),
            Action::Unblock(username) => self.unblock(uid, username).map(|_| ActionRes::Ok),
            Action::Logs => self.logs(uid).map(ActionRes::Logs),
        };

        info!(uid => "action {} => {:?}", cmd, res.as_ref().map(|_| ()));
        res
    }

    fn unblock(&mut self, uid: UserId, username: &str) -> anyhow::Result<()> {
        if uid != ADMIN_ID {
            anyhow::bail!("only admin can unblock the user")
        } else {
            self.users.unblock(username)
        }
    }

    pub fn pack(self) -> SystemImage {
        let logger = log::take_logger();

        let System { fs, users } = self;
        SystemImage { fs, users, logger }
    }

    pub fn logs(&self, uid: UserId) -> anyhow::Result<Vec<Log>> {
        if uid == ADMIN_ID {
            Ok(log::logger().logs().to_owned())
        } else {
            anyhow::bail!("only admin can view the logs")
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemImage {
    fs: Fs,
    users: UserDb,
    logger: Logger,
}

impl SystemImage {
    pub fn unpack(self) -> System {
        let SystemImage { fs, users, logger } = self;

        log::set_logger(logger);

        System { fs, users }
    }
}
