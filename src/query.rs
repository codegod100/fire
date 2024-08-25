use libsql::{params::IntoParams, Row};
use rocket::futures::{future, StreamExt};
use serde::{Deserialize, Serialize};
use anyhow::Result;

use crate::Turso;

#[derive(Debug, Serialize, Deserialize)]
pub struct Post {
    id: i32,
    title: String,
    body: String,
    author: String,
    #[serde(skip_deserializing)]
    comments: Vec<Comment>,
}



#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    id: i32,
    author: String,
    body: String,
    parent_id: Option<i32>,
    post_id: i32,
    #[serde(skip_deserializing)]
    children: Vec<Comment>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct User {
    id: i32,
    name: String,
}

impl User {
    pub async fn by_name(name: &str, t: &Turso) -> anyhow::Result<Option<User>> {
        let row = t
        .single_query(
            "select * from users where name = ?1",
            libsql::params! {name},
        )
        .await;
    Self::from_row(&row)
    }
}

trait FromRow {
    fn from_row<'de, T: Deserialize<'de>>(
        row: &'de Option<Row>,
    ) -> anyhow::Result<Option<T>> {
        match row {
            Some(row) => match libsql::de::from_row::<T>(row) {
                Ok(t) => Ok(Some(t)),
                Err(e) => Err(e)?,
            },
            None => Ok(None),
        }
    }
}

impl FromRow for Post {}
impl FromRow for User {}

impl Turso {
    pub async fn get_post_by_id(&self, id: i32) -> Option<Post> {
        let row = self
            .single_query("select * from posts where id = ?1", libsql::params! {id})
            .await;
        match row {
            Some(row) => {
                let mut post = libsql::de::from_row::<Post>(&row).unwrap();
                post.comments = sort_comments(self.get_comments_by_post_id(id).await);

                Some(post)
            }
            None => None,
        }
    }

    pub async fn get_comments_by_post_id(&self, id: i32) -> Vec<Comment> {
        let rows = self
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
            .map(|row| libsql::de::from_row(&row.unwrap()).unwrap())
            .collect()
            .await;
        comments
    }

    // pub async fn get_user_by_name(&self, name: &str) -> Result<Option<User>, serde::de::value::Error> {
    //     let row = self
    //         .single_query(
    //             "select * from users where name = ?1",
    //             libsql::params! {name},
    //         )
    //         .await;
    //     User::from_row(&row)
    // }

    async fn single_query(&self, q: &str, params: impl IntoParams) -> Option<Row> {
        self.0.query(q, params).await.unwrap().next().await.unwrap()
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
