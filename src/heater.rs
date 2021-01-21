use futures::future;
use futures::{stream, StreamExt};
use reqwest::{Client, IntoUrl, Response, StatusCode};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HeatingError {
    #[error("HTTP error")]
    RequestError(#[from] reqwest::Error),
}

pub async fn heat<T: IntoUrl>(iter: impl Iterator<Item = T>) -> Result<(), HeatingError> {
    let client = Client::new();

    let bodies = stream::iter(iter)
        .map(|url| {
            let client = &client;
            async move {
                let resp = client.get(url).send().await?;
                resp.bytes().await
            }
        })
        .buffer_unordered(num_cpus::get());

    bodies
        .for_each(|b| async {
            match b {
                Ok(b) => println!("Got {} bytes", b.len()),
                Err(e) => eprintln!("Got an error: {}", e),
            }
        })
        .await;

    Ok(())
}
