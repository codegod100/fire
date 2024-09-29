use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::Supa;

#[derive(Debug, Serialize, Deserialize)]
pub struct Post {
    pub id: i32,
    title: String,
    body: String,
    author: String,
    pub comments: Option<Vec<Comment>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    id: i32,
    pub created_at: String,
    pub newness: Option<i64>,
    pub newness_str: Option<String>,
    pub author: String,
    pub body: String,
    pub parent_id: Option<i32>,
    post_id: i32,
    comments: Option<Vec<Comment>>,
}

impl Comment {
    pub fn yolo(&self) -> String {
        "yolo".to_string()
    }
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

pub fn sort_comments(mut comments: Vec<Comment>) -> Result<Vec<Comment>> {
    let c = comments.to_owned();
    for comment in comments.iter_mut() {
        add_children(comment, &c)?;
    }

    Ok(comments
        .into_iter()
        .filter(|comment| match comment.parent_id {
            Some(_) => false,
            None => true,
        })
        .collect())
}

fn add_children(comment: &mut Comment, comments: &[Comment]) -> Result<()> {
    let mut children = children_for_parent(&comment, comments);
    for child in children.iter_mut() {
        add_children(child, comments);
    }
    let time = DateTime::parse_from_str(&comment.created_at, "%Y-%m-%dT%H:%M:%S%.6f%z")?;
    let now = Utc::now();
    let diff = now.signed_duration_since(time).num_seconds();
    comment.newness = Some(diff);
    comment.newness_str = Some(chrono_humanize::HumanTime::from(time).to_string());
    comment.comments = Some(children);
    Ok(())
}

fn children_for_parent(parent: &Comment, comments: &[Comment]) -> Vec<Comment> {
    let children = comments
        .into_iter()
        .filter(|comment| comment.parent_id == Some(parent.id))
        .map(|comment| comment.to_owned())
        .collect();
    children
}

impl Supa {
    pub async fn get_post(&self, id: i32) -> Result<Post> {
        let post = self
            .0
            .from("posts")
            .eq("id", id.to_string())
            .select("*")
            .single()
            .execute()
            .await
            .context("getting post")?;
        // let post = post.text().await.unwrap();
        let mut post = post.json::<Post>().await?;
        let comments = self
            .0
            .from("comments")
            .eq("post_id", post.id.to_string())
            .select("*")
            .order("created_at.desc")
            .execute()
            .await
            .context("getting comments")?;
        let comments = comments.json::<Vec<Comment>>().await?;
        let comments = sort_comments(comments)?;
        post.comments = Some(comments);
        Ok(post)
    }
}
