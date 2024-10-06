#[macro_use]
extern crate rocket;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use dotenvy::dotenv;
use postgrest::Postgrest;
use query::{Comment, User, UserForm};
use reqwest::Response;
use rocket::form::Form;
use rocket::http::{CookieJar, Status};
use rocket::request::FlashMessage;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::response::{Flash, Redirect};
use rocket::{Build, FromForm, Rocket, State};
use rocket_dyn_templates::{context, Template};
use serde::Serialize;
use std::env;
use std::fmt::Display;
use tracing::instrument;

use rocket::fs::NamedFile;
use std::path::{Path, PathBuf};

mod query;
mod supa;

struct Auth(String);

struct Supa(Postgrest);

#[derive(FromForm, Default, Serialize)]
pub struct CommentForm {
    pub author: Option<String>,
    pub post_id: Option<i32>,
    body: Option<String>,
    parent_id: Option<i32>,
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

// #[rocket::async_trait]
// impl<'r> FromRequest<'r> for Supa {
//     type Error = std::convert::Infallible;

//     async fn from_request(_request: &'r Request<'_>) -> Outcome<Supa, Self::Error> {
//         let client = Postgrest::new(env::var("SUPA_URL").unwrap())
//             .insert_header("apikey", env::var("SUPA_API_KEY").unwrap());
//         Outcome::Success(Supa(client))
//     }
// }

#[derive(Responder)]
enum Error {
    Err(String),
}

impl<T: Display + std::fmt::Debug> From<T> for Error {
    fn from(e: T) -> Self {
        error!("{e:?}");
        Error::Err(e.to_string())
    }
}

// fn nested_comments(depth: i32) -> String {
//     match depth {
//         1 => "comments(*)".to_string(),
//         _ => format!("comments(*, {})", nested_comments(depth - 1)),
//     }
// }

#[get("/")]
async fn index(auth: Auth, supa: &State<Supa>) -> Result<Template, Error> {
    // let query = format!("*, {}", nested_comments(5));
    let post = supa.get_post(1).await.context("while getting post")?;
    let template = Template::render(
        "index",
        context! {
            post: post,
            name: auth.0,

        },
    );
    Ok(template)
}

#[get("/", rank = 2)]
fn fallback_index(flash: Option<FlashMessage<'_>>) -> Template {
    match flash {
        Some(flash) => Template::render(
            "login",
            context! {
                message: flash.message(),
                kind: flash.kind()
            },
        ),
        None => Template::render("login", context! {}),
    }
}

#[get("/test")]
async fn test_path(supa: &State<Supa>) -> String {
    let comments = supa
        .select("comments", "*")
        .await
        .unwrap()
        .json::<Vec<Comment>>()
        .await
        .unwrap();
    format!("Results: {:#?}", comments)
}

#[get("/reply_comment/<post_id>/<comment_id>")]
async fn reply_comment(post_id: i32, comment_id: i32, auth: Auth) -> Template {
    Template::render(
        "reply_comment",
        context! {name: auth.0, post_id: post_id, comment_id: comment_id},
    )
}

#[get("/get_comment/<id>")]
async fn get_comment(id: i32, supa: &State<Supa>) -> Result<Template, Error> {
    let comment = supa
        .0
        .from("comments")
        .eq("id", id.to_string())
        .select("*")
        .single()
        .execute()
        .await?;
    let comment = comment.json::<Comment>().await?;
    let template = Template::render("edit_comment", context! {comment: comment});
    Ok(template)
}

#[instrument(skip(supa), ret)]
async fn insert_comment(supa: &Supa, comment: &str) -> Result<Response, reqwest::Error> {
    supa.0.from("comments").insert(comment).execute().await
}
#[post("/create_comment", data = "<comment>")]
async fn create_comment(
    supa: &State<Supa>,
    comment: Form<CommentForm>,
    auth: Auth,
) -> Result<Template, Error> {
    let comment = comment.into_inner();
    let comment = serde_json::to_string(&comment)?;
    info!("the comment is {comment:?}");
    insert_comment(supa, &comment).await?;
    info!("comment inserted");
    let post = supa.get_post(1).await?;
    let template = Template::render(
        "comments",
        context! {
            post: post,
            name: auth.0

        },
    );
    Ok(template)
}
#[post("/delete_comment/<id>")]
async fn delete_comment(id: i32, supa: &State<Supa>, auth: Auth) -> Result<Template, Error> {
    supa.0
        .from("comments")
        .eq("id", id.to_string())
        .delete()
        .execute()
        .await?;
    let post = supa.get_post(1).await?;
    let template = Template::render(
        "comments",
        context! {
            post: post,
            name: auth.0

        },
    );
    Ok(template)
}

#[post("/update_comment/<id>", data = "<comment>")]
async fn update_comment(
    id: &str,
    comment: Form<CommentForm>,
    supa: &State<Supa>,
) -> Result<Template, Error> {
    let now = Utc::now();

    let body = comment.body.clone().unwrap_or_default();
    let body = format!(r#"{{"body": "{}"}}"#, body.to_owned());
    let comment = supa
        .0
        .from("comments")
        .eq("id", id)
        .update(body)
        .select("*")
        .single()
        .execute()
        .await?;
    let mut comment = comment.json::<Comment>().await?;
    let time = DateTime::parse_from_str(&comment.created_at, "%Y-%m-%dT%H:%M:%S%.6f%z")?;
    let diff = now.signed_duration_since(time).num_seconds();
    comment.newness = Some(diff);
    comment.newness_str = Some(chrono_humanize::HumanTime::from(time).to_string());
    let template = Template::render("saved_comment", context! {comment: comment});
    Ok(template)
}

#[instrument(skip(supa), ret)]
async fn query_user(user: &Form<UserForm>, supa: &State<Supa>) -> Result<Response> {
    let response = supa
        .0
        .from("users")
        .eq("name", &user.name)
        .select("*")
        .single()
        .execute()
        .await?;
    Ok(response)
}
#[post("/", data = "<user>")]
async fn post_login(
    jar: &CookieJar<'_>,
    user: Form<UserForm>,
    supa: &State<Supa>,
) -> Result<Flash<Redirect>, Error> {
    info!("querying {user:?}");
    let user2 = query_user(&user, supa).await?;
    let user = query_user(&user, supa).await?;
    info!("{:?}", user2.text().await);
    let user = user.json::<User>().await;
    match user {
        Ok(user) => {
            info!("successfully got user {user:?}");
            jar.add_private(("user_id", user.name));
            Ok(Flash::success(Redirect::to(uri!(index)), ""))
        }
        Err(_) => Ok(Flash::error(Redirect::to(uri!(index)), "User not found")),
    }
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

fn build_rocket() -> Result<Rocket<Build>> {
    let build = rocket::build()
        .manage(Supa(
            Postgrest::new(env::var("SUPA_URL").context("Missing SUPA_URL")?).insert_header(
                "apikey",
                env::var("SUPA_API_KEY").context("Missing SUPA_API_KEY")?,
            ),
        ))
        .mount(
            "/",
            routes![
                index,
                fallback_index,
                get_comment,
                update_comment,
                create_comment,
                reply_comment,
                delete_comment,
                post_login,
                logout,
                test_path,
                files
            ],
        )
        .attach(Template::fairing());
    Ok(build)
}

#[rocket::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    match dotenv() {
        Ok(r) => println!("loaded {:#?}", r),
        Err(e) => println!(".env not found, skipping {}", e),
    }
    match build_rocket() {
        Ok(build) => {
            build.launch().await.unwrap();
            Ok(())
        }
        Err(e) => {
            error!("{e:?}");
            Err(e)
        }
    }
}
