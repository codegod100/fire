#[macro_use]
extern crate rocket;
use std::env;

use rocket::futures::{future, StreamExt};
use rocket::http::CookieJar;
use rocket::response::Redirect;
use rocket_dyn_templates::{context, Template};
use rocket::serde::{Serialize, Deserialize};
use rocket::http::Status;
use rocket::outcome::{try_outcome, IntoOutcome};
use rocket::request::{self, FromRequest, Outcome, Request};

use libsql::Builder;

use rocket::fs::NamedFile;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
struct Post {
    id: i32,
    title: String,
    body: String,
    author: String,
    comments: Vec<Comment>,
}

impl From<libsql::Row> for Post {
    fn from(row: libsql::Row) -> Self {
        Post {
            id: row.get(0).unwrap(),
            title: row.get(1).unwrap(),
            body: row.get(2).unwrap(),
            author: row.get(3).unwrap(),
            comments: vec![],
        }
    }
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

impl From<libsql::Row> for Comment {
    fn from(row: libsql::Row) -> Self {
        println!("[ROW]:: {:#?}", row);
        Comment {
            id: row.get(0).unwrap(),
            author: row.get(1).unwrap(),
            body: row.get(2).unwrap(),
            parent_id: row.get(3).unwrap(),
            post_id: row.get(4).unwrap(),
            children: vec![],
        }
    }
}

// impl From<libsql::Row> for Post {
//     fn from(row: libsql::Row) -> Self {
//         Post {
//             id: row.get(0).unwrap(),
//             title: row.get(1).unwrap(),
//             body: row.get(2).unwrap(),
//             author: row.get(3).unwrap(),
//             comments: vec![]
//         }
//     }
// }

// pub trait Convert {
//     fn convert(&self, row: libsql::Row) -> String {
//         format!("yolo")
//     }
// }

// impl Convert for Comment {}

// impl<T: Convert> ToString for T {
//     fn to_string(&self) -> String {
//         format!("yolo")
//     }

// }

struct Turso(libsql::Connection);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Turso {
    type Error = std::convert::Infallible;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Turso, Self::Error> {
        let url = env::var("LIBSQL_URL").expect("LIBSQL_URL must be set");
        let token = env::var("LIBSQL_AUTH_TOKEN").unwrap_or_default();

        let mut db = Builder::new_remote_replica("local.db", url, token)
            .build()
            .await
            .unwrap();
        db.sync().await.unwrap();
        let conn = db.connect().unwrap();
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
// #[derive(Debug)]
// struct Post {
//     id: i32,
//     name: String,
// }

// impl From<libsql::Row> for Post {
//     fn from(row: libsql::Row) -> Self {
//         Post {
//             id: row.get(0).unwrap(),
//             name: row.get(1).unwrap(),
//         }
//     }
// }

// impl FromIterator<libsql::Row> for Animal{
//     fn from_iter<T: IntoIterator<Item = libsql::Row>>(iter: T) -> Self {

//     }
// }

#[get("/")]
async fn index(jar: &CookieJar<'_>, turso: Turso) -> Template {
    let row = turso
        .0
        .query("select * from posts where id = ?1", libsql::params! {1})
        .await
        .unwrap()
        .next()
        .await
        .unwrap()
        .unwrap();
    let mut post = Post::from(row);
    println!("[ROWS]:: {:#?}", post);

    let rows = turso
        .0
        .query(
            "select * from comments where post_id = ?1",
            libsql::params! {1},
        )
        .await
        .unwrap();
    let comments: Vec<Comment> = rows
        .into_stream()
        .filter(|row| future::ready(row.is_ok()))
        .map(|row| {
            // println!("[ROW]:: {:#?}", row);
            // Comment::from(row.unwrap())
            libsql::de::from_row(&row.unwrap()).unwrap()
        })
        .collect()
        .await;
    println!("[ROWS]:: {:#?}", comments);

    let sorted = sort_comments(comments);
    post.comments = sorted;

    println!("[POST]:: {:#?}", post);

    // match jar.get_private("user_id") {
    //     Some(c) => {
    //         let p = sqlx::query_as::<_, Post>("select * from posts where id = ?")
    //             .bind(1)
    //             .fetch_one(&mut **db)
    //             .await;
    //         let co = sqlx::query_as::<_, Comment>("select * from comments where post_id = ?")
    //             .bind(1)
    //             .fetch_all(&mut **db)
    //             .await
    //             .unwrap();
    //         match p {
    //             Ok(mut post) => {
    //                 post.comments = sort_comments(co);
    //                 println!("[DATA]:: {:#?}", post);
    //                 Template::render(
    //                     "index",
    //                     context! {
    //                         name: c.value(),
    //                         post: post
    //                     },
    //                 )
    //             }
    //             Err(d) => {
    //                 println!("[ERROR]:: {}", d);
    //                 Template::render("login", context! {})
    //             }
    //         }
    //     }
    //     None => Template::render("login", context! {}),
    // }
    Template::render("index", context! {
        post: post,
        name: "admin"
    })
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

// #[get("/greeter")]
// fn greeter(jar: &CookieJar<'_>) -> Result<Template, Redirect> {
//     match jar.get_private("user_id") {
//         Some(c) => Ok(Template::render(
//             "index",
//             context! {
//                 name: c.value()
//             },
//         )),
//         None => Err(Redirect::to(uri!(index))),
//     }
// }

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
        .mount("/", routes![index, post_login, logout, test,files])
        .attach(Template::fairing())
}
