use futures::future;
use futures::{stream, StreamExt};
use reqwest::{Client, IntoUrl, Response, StatusCode};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HeatingError {
    #[error("HTTP error")]
    RequestError(#[from] reqwest::Error),
}

pub async fn heat<T: 'static + IntoUrl + Send>(
    iter: impl Iterator<Item = T>,
) -> Result<(), HeatingError> {
    let client = Client::new();

    let bodies = stream::iter(iter)
        .map(|url| {
            let client = client.clone();
            tokio::spawn(async move {
                let resp = client.get(url).send().await?;
                resp.bytes().await
            })
        })
        .buffer_unordered(num_cpus::get());

    bodies
        .for_each(|b| async {
            match b {
                Ok(Ok(b)) => println!("Got {} bytes", b.len()),
                Ok(Err(e)) => eprintln!("Got a reqwest::Error: {}", e),
                Err(e) => eprintln!("Got a tokio::JoinError: {}", e),
            }
        })
        .await;

    Ok(())
}
