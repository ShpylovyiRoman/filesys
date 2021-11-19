use filesys::{
    protocol::{ApiKey, IntoSerialize, LoginInfo, ResResult},
    users::UserId,
    Action, ActionRes, System,
};
use rocket::{
    futures::lock::Mutex,
    get,
    http::{Cookie, CookieJar},
    post, routes,
    serde::json::Json,
    State,
};

type Sys = Mutex<System>;

async fn login(sys: &System, creds: &LoginInfo, cookies: &CookieJar<'_>) -> anyhow::Result<UserId> {
    let uid = sys.login(&creds.username, &creds.password)?;
    let api_key = ApiKey::new(uid);
    let api_key = serde_json::to_string(&api_key)?;
    cookies.add_private(Cookie::new("apikey", api_key));
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
    creds: Json<LoginInfo>,
    cookies: &CookieJar<'_>,
) -> Json<ResResult<UserId>> {
    let sys = sys.lock().await;
    let res = login(&sys, &creds, cookies).await.into_serialize();
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

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[rocket::main]
async fn main() -> anyhow::Result<()> {
    let sys = Sys::new(System::new()?);

    rocket::build()
        .manage(sys)
        .mount("/", routes![index, login_endpoint, exec_endpoint])
        .launch()
        .await?;

    Ok(())
}
