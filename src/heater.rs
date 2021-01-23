use crate::config::Config;
use std::collections::HashSet;
use counter::Counter;
use futures::{stream, StreamExt};
use histogram::Histogram;
use reqwest::{header, Client, IntoUrl, StatusCode};
use std::time::{Duration, Instant};

pub async fn heat<T: 'static + IntoUrl + Send>(
    urls: impl Iterator<Item = T>,
) -> (Counter<StatusCode>, Histogram) {
    let client = Client::new();
    let config = Config::get();

    let stats: Vec<_> = stream::iter(urls)
        .map(|url| {
            let client = client.clone();
            tokio::spawn(async move { heat_one(&client, url).await })
        })
        .buffer_unordered(config.concurrent_requests)
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

async fn heat_one<T: 'static + IntoUrl + Send>(
    client: &Client,
    url: T,
) -> Result<(StatusCode, Duration), reqwest::Error> {
    let start = Instant::now();

    match client.get(url).send().await {
        Ok(response) => {
            let duration = start.elapsed();

            if let Some(headervalue) = response.headers().get(header::VARY) {
                if let Ok(value) = headervalue.to_str() {
                    for name in value.split(',') {
                        log::warn!("vary: {:?}", name.trim());
                    }
                }
            }

            Ok((response.status(), duration))
        }
        Err(err) => Err(err),
    }
}
