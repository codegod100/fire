use std::borrow::Cow;

use crate::Turso;
use anyhow::Result;
use libsql::de::from_row;
use libsql::{params::IntoParams, Row};
use rocket::futures::{future, StreamExt};
use serde::{Deserialize, Serialize};

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
    pub author: String,
    pub body: String,
    parent_id: Option<i32>,
    post_id: i32,
    #[serde(skip_deserializing)]
    children: Vec<Comment>,
}

#[derive(Serialize, Deserialize, Debug, FromForm)]
pub struct User {
    id: i32,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, FromForm)]
pub struct UserForm {
    pub name: String,
}

impl Turso {
    pub async fn get_post_by_id(&self, id: i32) -> Result<Option<Post>> {
        let row = self
            .single_query("select * from posts where id = ?1", libsql::params! {id})
            .await?;
        match row {
            Some(row) => {
                let mut post = from_row::<Post>(&row).unwrap();
                post.comments = sort_comments(self.get_comments_by_post_id(id).await?);

                Ok(Some(post))
            }
            None => Ok(None),
        }
    }
    pub async fn get_comment_by_id(&self, id: i32) -> Result<Option<Comment>> {
        let row = self
            .single_query("select * from comments where id = ?1", libsql::params! {id})
            .await?;
        match row {
            Some(row) => {
                let comment = from_row::<Comment>(&row).unwrap();
                Ok(Some(comment))
            }
            None => Ok(None),
        }
    }
    pub async fn update_comment(&self, id: i32, body: &str) -> Result<Option<Comment>> {
        self.0
            .execute(
                "update comments set body = ?1 where id = ?2",
                libsql::params! { body,id },
            )
            .await
            .unwrap();
        self.get_comment_by_id(id).await
    }

    pub async fn get_comments_by_post_id(&self, id: i32) -> Result<Vec<Comment>> {
        let rows = self
            .0
            .query(
                "select * from comments where post_id = ?1",
                libsql::params! {id},
            )
            .await?;
        let comments = rows
            .into_stream()
            .map(|row| {
                let row = row.unwrap();
                let comment = from_row::<Comment>(&row).unwrap();
                comment
            })
            .collect::<Vec<Comment>>()
            .await;
        Ok(comments)
    }

    pub async fn get_user_by_name(&self, name: &str) -> Result<Option<User>> {
        let row = self
            .single_query(
                "select * from users where name = ?1",
                libsql::params! {name},
            )
            .await?;
        match row {
            Some(row) => Ok(Some(from_row(&row)?)),
            None => Ok(None),
        }
    }

    async fn single_query(&self, q: &str, params: impl IntoParams) -> Result<Option<Row>> {
        let mut rows = self.0.query(q, params).await?;
        Ok(rows.next().await?)
    }
}

fn sort_comments(mut comments: Vec<Comment>) -> Vec<Comment> {
    let c = comments.to_owned();
    for comment in comments.iter_mut() {
        add_children(comment, &c)
    }

    comments
        .into_iter()
        .filter(|comment| match comment.parent_id {
            Some(_) => false,
            None => true,
        })
        .collect()
}

fn add_children(comment: &mut Comment, comments: &[Comment]) {
    let mut children = children_for_parent(&comment, comments);
    for child in children.iter_mut() {
        add_children(child, comments);
    }
    comment.children = children;
}

fn children_for_parent(parent: &Comment, comments: &[Comment]) -> Vec<Comment> {
    let children = comments
        .into_iter()
        .filter(|comment| comment.parent_id == Some(parent.id))
        .map(|comment| comment.to_owned())
        .collect();
    children
}
