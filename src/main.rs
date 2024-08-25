#[macro_use]
extern crate rocket;
use std::env;
use std::time::SystemTime;

use query::Post;
use rocket::http::CookieJar;
use rocket::request::{self, FromRequest, Outcome, Request};
use rocket::response::Redirect;
use rocket::serde::{Deserialize, Serialize};
use rocket::time::Instant;
use rocket_dyn_templates::{context, Template};

use libsql::Builder;

use rocket::fs::NamedFile;
use std::path::{Path, PathBuf};

mod query;

struct Turso(libsql::Connection);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Turso {
    type Error = std::convert::Infallible;

    async fn from_request(_request: &'r Request<'_>) -> request::Outcome<Turso, Self::Error> {
        let time = Instant::now();
        let url = env::var("LIBSQL_URL").expect("LIBSQL_URL must be set");
        let token = env::var("LIBSQL_AUTH_TOKEN").unwrap_or_default();

        let db = Builder::new_remote_replica("local.db", url, token)
            .build()
            .await
            .unwrap();
        let conn = db.connect().unwrap();
        db.sync().await.unwrap();
        println!("Time: {}", time.elapsed().as_seconds_f32());
        Outcome::Success(Turso(conn))
    }
}

#[derive(Clone)]
struct User(String);

#[get("/")]
async fn index(jar: &CookieJar<'_>, turso: Turso) -> Template {
    let post = Post::by_id(1, &turso).await.unwrap();
    match jar.get_private("user_id") {
        None => Template::render("login", context! {}),
        Some(c) => Template::render(
            "index",
            context! {
                post: post,
                name: c.value()
            },
        ),
    }
}

#[get("/test")]
fn test() -> String {
    format!("yolo")
}

#[post("/")]
fn post_login(jar: &CookieJar<'_>) -> Redirect {
    jar.add_private(("user_id", "admin"));
    Redirect::to(uri!(index))
}

#[post("/logout")]
fn logout(jar: &CookieJar<'_>) -> Redirect {
    jar.remove_private("user_id");
    Redirect::to(uri!(index))
}

#[get("/<file..>")]
async fn files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("static").join(file)).await.ok()
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![index, post_login, logout, test, files])
        .attach(Template::fairing())
}
