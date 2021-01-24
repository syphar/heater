use crate::config::Config;
use counter::Counter;
use futures::{stream, StreamExt};
use histogram::Histogram;
use reqwest::{header, Client, IntoUrl, StatusCode};
use std::collections::HashSet;
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
            result // while tokio join error can always panic, request errors shouldn't
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

async fn heat_one<T: IntoUrl>(
    client: &Client,
    url: T,
) -> Result<(StatusCode, Duration), reqwest::Error> {
    let start = Instant::now();

    let config = Config::get();

    let mut request = client.get(url);
    for (header, value) in config.header_variations.iter() {
        request = request.header(header, value);
    }

    match request.send().await {
        Ok(response) => {
            let duration = start.elapsed();

            // log a warning if the `Vary` header contains of values which
            // are not defined in the header variations.
            for headervalue in response.headers().get_all(header::VARY) {
                if let Ok(value) = headervalue.to_str() {
                    let headers_in_request: HashSet<header::HeaderName> = value
                        .split(',')
                        .map(|v| v.trim())
                        .map(|s| s.parse())
                        .filter_map(Result::ok)
                        .collect();

                    let configured_headers: HashSet<header::HeaderName> =
                        config.header_variations.keys().cloned().collect();

                    for missing in headers_in_request.difference(&configured_headers) {
                        log::warn!("received Vary header '{}' that is missing in configured header variations", missing);
                    }
                }
            }

            Ok((response.status(), duration))
        }
        Err(err) => Err(err),
    }
}
