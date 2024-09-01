#[macro_use]
extern crate rocket;
use dotenvy::dotenv;
use libsql::Builder;
use postgrest::Postgrest;
use query::{Comment, Post, User, UserForm};
use rocket::form::Form;
use rocket::http::{CookieJar, Status};
use rocket::request::FlashMessage;
use rocket::request::{self, FromRequest, Outcome, Request};
use rocket::response::{Flash, Redirect};
use rocket_dyn_templates::{context, Metadata, Template};
use serde::Deserialize;
use std::time::Instant;
use std::{env, io};

use rocket::fs::NamedFile;
use std::path::{Path, PathBuf};

mod query;
mod supa;

struct Turso(libsql::Connection);

struct Auth(String);

struct Supa(Postgrest);

#[derive(FromForm, Default)]
pub struct CommentForm {
    id: Option<i32>,
    pub author: Option<String>,
    pub post_id: Option<i32>,
    body: Option<String>,
    parent_id: Option<i32>,
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

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Supa {
    type Error = std::convert::Infallible;

    async fn from_request(_request: &'r Request<'_>) -> Outcome<Supa, Self::Error> {
        let client = Postgrest::new(env::var("SUPA_URL").unwrap())
            .insert_header("apikey", env::var("SUPA_API_KEY").unwrap());
        Outcome::Success(Supa(client))
    }
}

#[derive(Responder)]
enum Error {
    DBFailure(String),
}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        Error::DBFailure(e.to_string())
    }
}

fn nested_comments(depth: i32) -> String {
    match depth {
        1 => "comments(*)".to_string(),
        _ => format!("comments(*, {})", nested_comments(depth - 1)),
    }
}

#[get("/")]
async fn index(auth: Auth, turso: Turso, supa: Supa) -> Template {
    println!("{}", auth.0);
    let query = format!("*, {}", nested_comments(5));
    println!("{query}");
    let post = supa
        .0
        .from("posts")
        .eq("author", &auth.0)
        .select(query)
        .single()
        .execute()
        .await
        .unwrap();
    // let post = post.text().await.unwrap();
    let mut post = post.json::<Post>().await.unwrap();
    let comments = post
        .comments
        .into_iter()
        .filter(|c| c.parent_id.is_none())
        .collect();
    post.comments = comments;
    println!("{:#?}", post);
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

#[derive(Deserialize, Debug)]
struct SupaComment {
    id: i32,
    body: String,
}
#[get("/test")]
async fn test_path(supa: Supa) -> String {
    let comments = supa
        .select("comments", "*")
        .await
        .unwrap()
        .json::<Vec<Comment>>()
        .await
        .unwrap();
    format!("Results: {:#?}", comments)
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

#[get("/reply_comment/<post_id>/<comment_id>")]
async fn reply_comment(post_id: i32, comment_id: i32, turso: Turso, auth: Auth) -> Template {
    Template::render(
        "reply_comment",
        context! {name: auth.0, post_id: post_id, comment_id: comment_id},
    )
}

#[post("/create_comment", data = "<comment>")]
async fn create_comment(turso: Turso, comment: Form<CommentForm>, auth: Auth) -> Template {
    turso.create_comment(&comment).await.unwrap();

    let posts = turso.get_posts_by_username(&auth.0).await.unwrap();
    let post = posts.into_iter().next().unwrap();

    Template::render(
        "comments",
        context! {
            post: post,
            name: auth.0

        },
    )
}

#[post("/update_comment", data = "<comment>")]
async fn update_comment(comment: Form<CommentForm>, turso: Turso) -> Template {
    let id = comment.id.clone().unwrap();
    let body = comment.body.clone().unwrap();
    let comment = turso.update_comment(id, &body).await.unwrap().unwrap();
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
async fn rocket() -> _ {
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
                create_comment,
                reply_comment,
                post_login,
                logout,
                test_path,
                files
            ],
        )
        .attach(Template::fairing())
}
