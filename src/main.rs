#[macro_use]
extern crate rocket;
use std::env;

use libsql::Builder;
use query::User;
use rocket::form::Form;
use rocket::http::CookieJar;
use rocket::request::FlashMessage;
use rocket::request::{self, FromRequest, Outcome, Request};
use rocket::response::{Flash, Redirect};
use rocket_dyn_templates::{context, Template};
use std::time::Instant;

use rocket::fs::NamedFile;
use std::path::{Path, PathBuf};

mod query;

struct Turso(libsql::Connection);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Turso {
    type Error = std::convert::Infallible;

    async fn from_request(_request: &'r Request<'_>) -> Outcome<Turso, Self::Error> {
        let time = Instant::now();
        let url = env::var("LIBSQL_URL").expect("LIBSQL_URL must be set");
        let token = env::var("LIBSQL_AUTH_TOKEN").unwrap_or_default();

        let db = Builder::new_remote_replica("local.db", url, token)
            .build()
            .await
            .unwrap();
        let conn = db.connect().unwrap();
        db.sync().await.unwrap();
        println!("Time: {}", time.elapsed().as_secs_f32());
        Outcome::Success(Turso(conn))
    }
}

#[get("/")]
async fn index(jar: &CookieJar<'_>, turso: Turso, flash: Option<FlashMessage<'_>>) -> Template {
    let post = turso.get_post_by_id(1).await.unwrap();
    match jar.get_private("user_id") {
        None => match flash {
            Some(flash) => {
                println!("flash: {:#?}", flash);

                Template::render(
                    "login",
                    context! {
                        message: flash.message(),
                        kind: flash.kind()
                    },
                )
            }
            None => Template::render("login", context! {}),
        },
        Some(c) => Template::render(
            "index",
            context! {
                post: post,
                name: c.value(),

            },
        ),
    }
}

#[get("/test")]
fn test() -> String {
    format!("yolo")
}

#[post("/", data = "<user>")]
async fn post_login(
    jar: &CookieJar<'_>,
    user: Form<User>,
    turso: Turso,
) -> Result<Redirect, Flash<Redirect>> {
    println!("USER: {:#?}", user);
    let db_user = turso.get_user_by_name(&user.name).await.unwrap();
    println!("DB_USER: {:#?}", db_user);
    if let Some(db_user) = db_user {
        if db_user.name == user.name {
            jar.add_private(("user_id", db_user.name));
            return Ok(Redirect::to(uri!(index)));
        }
    }
    Err(Flash::error(Redirect::to(uri!(index)), "User not found"))
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
