use futures::future;
use futures::{stream, StreamExt};
use log::debug;
use reqwest::{Client, IntoUrl, Response, StatusCode};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HeatingError {
    #[error("HTTP error")]
    RequestError(#[from] reqwest::Error),
}

pub async fn heat<T: 'static + IntoUrl + Send>(
    urls: impl Iterator<Item = T>,
) -> Result<(), HeatingError> {
    let client = Client::new();

    let stats = future::join_all(urls.map(|url| {
        let client = client.clone();
        tokio::spawn(async move {
            let start = Instant::now();

            match client.get(url).send().await {
                Ok(response) => Ok((response.status(), start.elapsed())),
                Err(err) => Err(err),
            }
        })
    }))
    .await;

    for s in stats {
        if let Ok(result) = s {
            match result {
                Ok((status, time)) => debug!("stats: {:?} / {:?}", status, time),
                Err(err) => debug!("error: {:?}", err),
            };
        }
    }

    Ok(())
}
