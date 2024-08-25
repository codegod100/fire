#[macro_use]
extern crate rocket;
use std::env;

use rocket::http::CookieJar;
use rocket::request::{self, FromRequest, Outcome, Request};
use rocket::response::Redirect;
use rocket::serde::{Deserialize, Serialize};
use rocket_dyn_templates::{context, Template};

use libsql::Builder;

use rocket::fs::NamedFile;
use std::path::{Path, PathBuf};

mod query;

#[derive(Debug, Serialize, Deserialize)]
struct Post {
    id: i32,
    title: String,
    body: String,
    author: String,
    #[serde(skip_deserializing)]
    comments: Vec<Comment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Comment {
    id: i32,
    author: String,
    body: String,
    parent_id: Option<i32>,
    post_id: i32,
    #[serde(skip_deserializing)]
    children: Vec<Comment>,
}

struct Turso(libsql::Connection);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Turso {
    type Error = std::convert::Infallible;

    async fn from_request(_request: &'r Request<'_>) -> request::Outcome<Turso, Self::Error> {
        let url = env::var("LIBSQL_URL").expect("LIBSQL_URL must be set");
        let token = env::var("LIBSQL_AUTH_TOKEN").unwrap_or_default();

        let db = Builder::new_remote_replica("local.db", url, token)
            .build()
            .await
            .unwrap();
        let conn = db.connect().unwrap();
        db.sync().await.unwrap();
        Outcome::Success(Turso(conn))
    }
}

#[derive(Clone)]
struct User(String);

fn sort_comments(comments: Vec<Comment>) -> Vec<Comment> {
    let c_comments = comments.clone();
    let parent_comments: Vec<Comment> = c_comments
        .into_iter()
        .filter(|c| c.parent_id == None)
        .map(|c| {
            let c_comments = comments.clone();
            add_children(c, c_comments)
        })
        .collect();
    parent_comments
}

fn add_children(mut comment: Comment, comments: Vec<Comment>) -> Comment {
    // Find children for comment
    let c_comment = comment.clone();
    let c_comments = comments.clone();
    let children = children_for_parent(c_comment, c_comments);
    let nested: Vec<Comment> = children
        .into_iter()
        .map(|child| {
            let c_comments = comments.clone();
            add_children(child, c_comments)
        })
        .collect();
    comment.children = nested;
    comment
}

fn children_for_parent(parent: Comment, comments: Vec<Comment>) -> Vec<Comment> {
    comments
        .into_iter()
        .filter(|comment| match comment.parent_id {
            Some(parent_id) => parent_id == parent.id,
            None => false,
        })
        .collect()
}

#[get("/")]
async fn index(jar: &CookieJar<'_>, turso: Turso) -> Template {
    let post = turso.get_post_by_id(1).await;

    println!("[POST]:: {:#?}", post);

    let c = jar.get_private("user_id");
    if let None = c {
        return Template::render("login", context! {});
    }

    Template::render(
        "index",
        context! {
            post: post,
            name: c.unwrap().value()
        },
    )
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
