#[macro_use]
extern crate rocket;
use rocket::http::{CookieJar, Status};
use rocket::response::{Flash, Redirect};
use rocket_db_pools::sqlx::sqlite::SqliteRow;
use rocket_dyn_templates::{context, tera::Tera, Template};
// use serde::Serialize;
use rocket_db_pools::sqlx::{self, Row};
use rocket_db_pools::{Connection, Database};
use serde::Deserialize;
use std::borrow::Cow;

#[derive(Database)]
#[database("main")]
struct Main(sqlx::SqlitePool);

#[derive(sqlx::FromRow, Debug, serde::Serialize)]
struct Post {
    id: i32,
    title: String,
    body: String,
    author: String,
    #[sqlx(skip)]
    comments: Vec<Comment>,
}

#[derive(sqlx::FromRow, Debug, serde::Serialize, Clone)]
struct Comment {
    id: i32,
    author: String,
    body: String,
    parent_id: Option<i32>,
    post_id: i32,
    #[sqlx(skip)]
    children: Vec<Comment>,
}

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
async fn index(mut db: Connection<Main>, jar: &CookieJar<'_>) -> Template {
    match jar.get_private("user_id") {
        Some(c) => {
            let p = sqlx::query_as::<_, Post>("select * from posts where id = ?")
                .bind(1)
                .fetch_one(&mut **db)
                .await;
            let co = sqlx::query_as::<_, Comment>("select * from comments where post_id = ?")
                .bind(1)
                .fetch_all(&mut **db)
                .await
                .unwrap();
            match p {
                Ok(mut post) => {
                    post.comments = sort_comments(co);
                    println!("[DATA]:: {:#?}", post);
                    Template::render(
                        "index",
                        context! {
                            name: c.value(),
                            post: post
                        },
                    )
                }
                Err(d) => {
                    println!("[ERROR]:: {}", d);
                    Template::render("login", context! {})
                }
            }
        }
        None => Template::render("login", context! {}),
    }
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
#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![index, post_login, logout])
        .attach(Template::fairing())
        .attach(Main::init())
}
