use rocket::futures::{future, StreamExt};

use crate::{sort_comments, Comment, Post, Turso};

impl Turso {
    pub async fn get_post_by_id(&self, id: i32) -> Post {
        let row = self
            .0
            .query("select * from posts where id = ?1", libsql::params! {id})
            .await
            .unwrap()
            .next()
            .await
            .unwrap()
            .unwrap();
        let mut post = libsql::de::from_row::<Post>(&row).unwrap();
        let comments = self.get_comments_by_post_id(id).await;
        let sorted = sort_comments(comments);
        post.comments = sorted;
        
        post
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
}
