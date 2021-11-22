use std::{io::ErrorKind, sync::Arc};

use filesys::{
    protocol::{ApiKey, IntoSerialize, LoginInfo, ResResult},
    users::UserId,
    Action, ActionRes, System, SystemImage,
};
use rocket::{
    futures::lock::Mutex,
    http::{Cookie, CookieJar},
    post, routes,
    serde::json::Json,
    time::Duration,
    State,
};
use structopt::StructOpt;

type Sys = Arc<Mutex<System>>;

async fn login(
    sys: &mut System,
    opt: &Opt,
    creds: &LoginInfo,
    cookies: &CookieJar<'_>,
) -> anyhow::Result<UserId> {
    let uid = sys.login(&creds.username, &creds.password)?;
    let api_key = ApiKey::new(uid);
    let api_key = serde_json::to_string(&api_key)?;
    let expires = rocket::time::OffsetDateTime::now_utc() + Duration::new(opt.api_exp_sec, 0);

    let cookie = Cookie::build("apikey", api_key)
        .expires(Some(expires))
        .finish();
    cookies.add_private(cookie);
    Ok(uid)
}

fn get_api_key(cookies: &CookieJar<'_>) -> anyhow::Result<ApiKey> {
    cookies
        .get_private("apikey")
        .and_then(|cookie| serde_json::from_str(cookie.value()).ok())
        .ok_or_else(|| anyhow::anyhow!("authentication required"))
}

#[post("/login", format = "json", data = "<creds>")]
async fn login_endpoint(
    sys: &State<Sys>,
    opt: &State<Opt>,
    creds: Json<LoginInfo>,
    cookies: &CookieJar<'_>,
) -> Json<ResResult<()>> {
    let mut sys = sys.lock().await;
    let res = login(&mut sys, opt, &creds, cookies)
        .await
        .into_serialize()
        .map(|_| ());
    Json(res)
}

async fn exec(
    sys: &mut System,
    cookies: &CookieJar<'_>,
    action: &Action,
) -> anyhow::Result<ActionRes> {
    let api_key = get_api_key(cookies)?;
    sys.exec(api_key.uid(), action)
}

#[post("/exec", format = "json", data = "<action>")]
async fn exec_endpoint(
    sys: &State<Sys>,
    cookies: &CookieJar<'_>,
    action: Json<Action>,
) -> Json<ResResult<ActionRes>> {
    let mut sys = sys.lock().await;
    let res = exec(&mut sys, cookies, &action).await.into_serialize();
    Json(res)
}

#[derive(Debug, structopt::StructOpt)]
struct Opt {
    image: String,

    #[structopt(long, default_value = "60")]
    api_exp_sec: i64,
}

#[rocket::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();

    let sys = match std::fs::File::open(&opt.image) {
        Err(err) if err.kind() == ErrorKind::NotFound => System::new()?,
        Ok(file) => {
            let image: SystemImage = bincode::deserialize_from(file)?;
            image.unpack()
        }
        Err(err) => return Err(err.into()),
    };

    let out = std::fs::File::create(&opt.image)?;

    let sys = Sys::new(Mutex::new(sys));

    rocket::build()
        .manage(sys.clone())
        .manage(opt)
        .mount("/", routes![login_endpoint, exec_endpoint])
        .launch()
        .await?;

    let sys = Arc::try_unwrap(sys)
        .map_err(|_| anyhow::anyhow!("bug: should be only one reference"))
        .unwrap();
    let sys = sys.into_inner().pack();
    bincode::serialize_into(out, &sys)?;

    Ok(())
}
