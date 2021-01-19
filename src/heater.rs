use reqwest::{Client, IntoUrl};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HeatingError {
    #[error("HTTP error")]
    RequestError(#[from] reqwest::Error),
}

pub async fn heat<T: IntoUrl>(iter: impl Iterator<Item = T>) -> Result<(), HeatingError> {
    let client = Client::new();

    for url in iter {
        heat_url(client.clone(), url).await?
    }

    Ok(())
}

async fn heat_url<T: IntoUrl>(client: Client, url: T) -> Result<(), HeatingError> {
    client.get(url).send().await?;
    Ok(())
}
