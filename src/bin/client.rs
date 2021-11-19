use std::{
    io::{self, Write},
    path::PathBuf,
};

use filesys::{
    fs::NodeTag,
    protocol::{self, EDeserialize, ResResult},
    users::{Perms, Username},
    Action, ActionRes,
};
use reqwest::{Client, ClientBuilder};
use structopt::{clap::AppSettings, StructOpt};

#[derive(Debug, structopt::StructOpt)]
struct Opt {
    #[structopt(long, default_value = "127.0.0.1")]
    host: String,

    #[structopt(long, default_value = "8000")]
    port: u16,
}

fn read_pass(prefix: &str) -> anyhow::Result<String> {
    print!("{}: ", prefix);
    io::stdout().flush()?;
    rpassword::read_password().map_err(Into::into)
}

fn read_uname_pass() -> anyhow::Result<(String, String)> {
    print!("login: ");
    io::stdout().flush()?;
    let mut login = String::new();
    io::stdin().read_line(&mut login)?;
    let len = login.trim_end().len();
    login.truncate(len);

    let pass = read_pass("password")?;
    Ok((login, pass))
}

#[derive(Debug, structopt::StructOpt)]
#[structopt(setting(AppSettings::NoBinaryName))]
enum Cmd {
    Read { path: PathBuf },
    Write { path: PathBuf, data: String },
    Rm { path: PathBuf },
    NewFile { path: PathBuf },
    NewDir { path: PathBuf },
    Exec { path: PathBuf },
    SetPerms { path: PathBuf, perms: Perms },
    Ls { path: PathBuf },
    AddUser { username: Username },
    ChangePass,
    Exit,
}

pub struct State {
    base: String,
    client: Client,
}

fn print_action_res(res: &ActionRes) {
    match res {
        ActionRes::Ok => {}
        ActionRes::Read(str) => println!("{}", str),
        ActionRes::Ls(entries) => {
            for entry in entries {
                let tag = match entry.tag {
                    NodeTag::File => 'f',
                    NodeTag::Dir => 'd',
                };
                println!("{}{} {:>4} {}", tag, entry.perms, entry.size, entry.name);
            }
        }
    }
}

impl State {
    async fn new(base: String, username: String, password: String) -> anyhow::Result<Self> {
        let client = ClientBuilder::new().cookie_store(true).build()?;

        let creds = protocol::LoginInfo { username, password };

        client
            .post(format!("{}/login", base))
            .json(&creds)
            .send()
            .await?
            .json::<ResResult<()>>()
            .await?
            .deserialize()?;

        Ok(Self { base, client })
    }

    async fn execute(&self, cmd: Cmd) -> anyhow::Result<bool> {
        let cmd = match cmd {
            Cmd::Read { path } => Action::Read(path),
            Cmd::Write { path, data } => Action::Write(path, data),
            Cmd::Rm { path } => Action::Rm(path),
            Cmd::NewFile { path } => Action::NewFile(path),
            Cmd::NewDir { path } => Action::NewDir(path),
            Cmd::Exec { path } => Action::Exec(path),
            Cmd::SetPerms { path, perms } => Action::SetPerms(path, perms),
            Cmd::Ls { path } => Action::Ls(path),
            Cmd::AddUser { username } => Action::AddUser(username),
            Cmd::ChangePass => {
                let old = read_pass("old password")?;
                let new = read_pass("new password")?;
                let new2 = read_pass("repeat new password")?;
                if new != new2 {
                    anyhow::bail!("passwords don't match");
                }
                Action::ChangePassword { old, new }
            }
            Cmd::Exit => return Ok(true),
        };

        let res = self
            .client
            .post(format!("{}/exec", self.base))
            .json(&cmd)
            .send()
            .await?
            .json::<ResResult<ActionRes>>()
            .await?
            .deserialize()?;

        print_action_res(&res);
        Ok(false)
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    let base = format!("http://{}:{}", opt.host, opt.port);

    let (username, password) = read_uname_pass()?;
    let state = State::new(base, username, password).await?;

    let mut line = String::new();
    loop {
        line.clear();
        print!("$ ");
        io::stdout().flush()?;
        io::stdin().read_line(&mut line)?;

        let parts = shellwords::split(&line)?;
        let cmd = match Cmd::from_iter_safe(parts) {
            Ok(cmd) => cmd,
            Err(err) => {
                eprintln!("{}", err);
                continue;
            }
        };
        match state.execute(cmd).await {
            Ok(exit) => {
                if exit {
                    break;
                }
            }
            Err(err) => eprintln!("{}", err),
        }
    }
    Ok(())
}
