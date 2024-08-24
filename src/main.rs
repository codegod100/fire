#[macro_use]
extern crate rocket;
use rocket::http::CookieJar;
use rocket::response::Redirect;
use rocket_dyn_templates::{context, Template};
// use serde::Serialize;
use rocket_db_pools::sqlx::{self};
use rocket_db_pools::{Connection, Database};

use rocket::http::Status;
use rocket::outcome::{try_outcome, IntoOutcome};
use rocket::request::{self, FromRequest, Outcome, Request};

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


#[derive(sqlx::FromRow)]
struct Person{
    id: i32,
    username: String
}

#[derive(Clone)]
struct User(String);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for User {
    type Error = std::convert::Infallible;

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<User, Self::Error> {
        let outcome = request.guard::<&Main>().await;
        let db = outcome.unwrap();
        let user_option = request.cookies()
        .get_private("user_id")
        .and_then(|cookie| cookie.value().parse().ok())
        .map(User);
    let u = user_option.clone().unwrap().0;
    let res = sqlx::query_as::<_, Person>("select * from users where username = ?").bind(u).fetch_one(&db.0).await;
        let m = match res{
            Ok(_) => user_option,
            Err(_) => None
        };
        m.or_forward(Status::Unauthorized)

    }
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
async fn index(mut db: Connection<Main>, jar: &CookieJar<'_>, ) -> Template {
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

#[get("/test")]
fn test(u: User) -> String {
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
#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![index, post_login, logout,test])
        .attach(Template::fairing())
        .attach(Main::init())
}
