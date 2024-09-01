use crate::Supa;
use anyhow::Result;
use reqwest::Response;

impl Supa {
    pub async fn select(&self, t: &str, s: &str) -> Result<Response> {
        let result = self.0.from(t).select(s).execute().await?;
        Ok(result)
    }
}
