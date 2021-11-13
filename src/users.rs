pub type Username = String;

pub struct Perms {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
    pub control: bool,
}

pub struct User {
    name: Username,
    pass: String,
}
