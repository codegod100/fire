#[macro_use]
extern crate rocket;
use dotenvy::dotenv;
use libsql::Builder;
use query::{Comment, User, UserForm};
use rocket::form::Form;
use rocket::http::{CookieJar, Status};
use rocket::request::FlashMessage;
use rocket::request::{self, FromRequest, Outcome, Request};
use rocket::response::{Flash, Redirect};
use rocket_dyn_templates::{context, Template};
use std::env;
use std::time::Instant;

use rocket::fs::NamedFile;
use std::path::{Path, PathBuf};

mod query;

struct Turso(libsql::Connection);

struct Auth(String);

#[derive(FromForm)]
pub struct CommentForm {
    id: i32,
    pub body: String,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Turso {
    type Error = std::convert::Infallible;

    async fn from_request(_request: &'r Request<'_>) -> Outcome<Turso, Self::Error> {
        let time = Instant::now();
        let url = env::var("LIBSQL_URL").expect("LIBSQL_URL must be set");
        let token = env::var("LIBSQL_AUTH_TOKEN").unwrap_or_default();

        // let db = Builder::new_remote_replica("local.db", url, token)
        //     .build()
        //     .await
        //     .unwrap();
        // let conn = db.connect().unwrap();
        let db = Builder::new_remote(url, token).build().await.unwrap();
        let conn = db.connect().unwrap();

        // db.sync().await.unwrap();
        println!("Time: {}", time.elapsed().as_secs_f32());
        Outcome::Success(Turso(conn))
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Auth {
    type Error = std::convert::Infallible;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Auth, Self::Error> {
        let jar = request.guard::<&CookieJar<'_>>().await.unwrap();
        match jar.get_private("user_id") {
            None => Outcome::Forward(Status::Unauthorized),
            Some(c) => Outcome::Success(Auth(c.value().to_string())),
        }
    }
}

#[get("/")]
async fn index(auth: Auth, turso: Turso) -> Template {
    let post = turso.get_post_by_id(1).await.unwrap();
    Template::render(
        "index",
        context! {
            post: post,
            name: auth.0,

        },
    )
}

#[get("/", rank = 2)]
fn fallback_index(flash: Option<FlashMessage<'_>>) -> Template {
    match flash {
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
    }
}
#[get("/test")]
fn test(_auth: Auth) -> String {
    format!("yolo")
}

#[get("/get_comment/<id>")]
async fn get_comment(id: i32, turso: Turso) -> Template {
    let comment = turso.get_comment_by_id(id).await.unwrap().unwrap();
    Template::render(
        "edit_comment",
        context! {
            comment: comment
        },
    )
}

#[post("/update_comment", data = "<comment>")]
async fn update_comment(comment: Form<CommentForm>, turso: Turso) -> Template {
    let comment = turso
        .update_comment(comment.id.clone(), comment.body.clone())
        .await
        .unwrap()
        .unwrap();
    Template::render("saved_comment", context! {comment: comment})
}

#[post("/", data = "<user>")]
async fn post_login(
    jar: &CookieJar<'_>,
    user: Form<UserForm>,
    turso: Turso,
) -> Result<Redirect, Flash<Redirect>> {
    let db_user = turso.get_user_by_name(&user.name).await.unwrap();
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

#[get("/static/<file..>")]
async fn files(file: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new("static").join(file)).await.ok()
}

#[launch]
fn rocket() -> _ {
    match dotenv() {
        Ok(r) => println!("loaded {:#?}", r),
        Err(e) => println!(".env not found, skipping {}", e),
    }
    rocket::build()
        .mount(
            "/",
            routes![
                index,
                fallback_index,
                get_comment,
                update_comment,
                post_login,
                logout,
                test,
                files
            ],
        )
        .attach(Template::fairing())
}
