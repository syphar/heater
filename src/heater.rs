use crate::config;
use counter::Counter;
use futures::{stream, StreamExt};
use histogram::Histogram;
use reqwest::{Client, IntoUrl, StatusCode};
use std::time::Instant;

pub async fn heat<T: 'static + IntoUrl + Send>(
    urls: impl Iterator<Item = T>,
) -> (Counter<StatusCode>, Histogram) {
    let client = Client::new();

    let stats: Vec<_> = stream::iter(urls)
        .map(|url| {
            let client = client.clone();
            tokio::spawn(async move {
                let start = Instant::now();

                match client.get(url).send().await {
                    Ok(response) => Ok((response.status(), start.elapsed())),
                    Err(err) => Err(err),
                }
            })
        })
        .buffer_unordered(*(config::CONCURRENT_REQUESTS))
        .map(|result| {
            result
                .unwrap_or_else(|err| panic!("tokio error: {:?}", err))
                .unwrap_or_else(|err| panic!("reqwest error error: {:?}", err))
        })
        .collect()
        .await;

    let counts = stats
        .iter()
        .map(|(status, _)| status)
        .cloned()
        .collect::<Counter<_>>();

    let mut histogram = Histogram::new();

    for (_, elapsed) in stats {
        histogram.increment(elapsed.as_millis() as u64).unwrap();
    }

    (counts, histogram)
}
