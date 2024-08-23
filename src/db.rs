use diesel::prelude::*;
use rocket_sync_db_pools::database;

#[database("diesel")]
struct Db(diesel::SqliteConnection);

#[derive(serde::Serialize, Queryable)]
#[diesel(table_name = posts)]
struct Post {
    #[serde(skip_serializing)]
    id: Option<i32>,
    title: String,
    body: String,
    author: String,
}

table! {
    posts (id) {
        id -> Nullable<Integer>,
        title -> Text,
        body -> Text,
        author -> Text,
    }
}

#[derive(serde::Serialize, Queryable)]
#[diesel(table_name = comments)]
struct Comment {
    #[serde(skip_serializing)]
    id: Option<i32>,
    author: String,
    body: String,
    parent_id: Option<i32>,
    post_id: i32,
}

table! {
    comments (id) {
        id -> Nullable<Integer>,
        author -> Text,
        body -> Text,
        parent_id -> Nullable<Integer>,
        post_id -> Integer
    }
}

// fn new_post() -> Post {
//     let comment = Comment {
//         author: String::from("me"),
//         body: String::from("yolo"),
//         children: vec![Comment {
//             author: String::from("blu"),
//             body: String::from("wee"),
//             children: vec![],
//         }],
//     };
//     Post {
//         title: String::from("hello"),
//         body: String::from("world"),
//         author: String::from("me"),
//         comments: vec![comment],
//     }
// }
//

async fn get_post(db: Db, id: i32) -> Result<Post, diesel::result::Error> {
    db.run(move |conn| posts::table.filter(posts::id.eq(id)).first(conn))
        .await
}
