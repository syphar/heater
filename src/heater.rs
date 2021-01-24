use crate::config::Config;
use counter::Counter;
use futures::{stream, StreamExt};
use histogram::Histogram;
use reqwest::{
    header::{self, HeaderName},
    Client, IntoUrl, StatusCode,
};
use std::collections::HashSet;
use std::time::{Duration, Instant};

pub async fn heat<T: 'static + IntoUrl + Send>(
    urls: impl Iterator<Item = T>,
) -> (Counter<StatusCode>, Counter<Option<bool>>, Histogram) {
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
        .map(|(status, _, _)| status)
        .cloned()
        .collect::<Counter<_>>();

    let cache_hits = stats
        .iter()
        .map(|(_, cache_hit, _)| cache_hit)
        .cloned()
        .collect::<Counter<_>>();

    let mut histogram = Histogram::new();

    for (_, _, elapsed) in stats {
        histogram.increment(elapsed.as_millis() as u64).unwrap();
    }

    (counts, cache_hits, histogram)
}

async fn heat_one<T: IntoUrl>(
    client: &Client,
    url: T,
) -> Result<(StatusCode, Option<bool>, Duration), reqwest::Error> {
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
                    let headers_in_request: HashSet<HeaderName> = value
                        .split(',')
                        .map(|v| v.trim())
                        .map(|s| s.parse())
                        .filter_map(Result::ok)
                        .collect();

                    let configured_headers: HashSet<HeaderName> =
                        config.header_variations.keys().cloned().collect();

                    for missing in headers_in_request.difference(&configured_headers) {
                        log::warn!("received Vary header '{}' that is missing in configured header variations", missing);
                    }
                }
            }

            let cache_hit = if let Some(headervalue) =
                response.headers().get(HeaderName::from_static("x-cache"))
            {
                if let Ok(value) = headervalue.to_str() {
                    Some(value[0..3].to_lowercase() == "hit")
                } else {
                    None
                }
            } else {
                None
            };
            // if let Some(value) = response.headers().get(

            Ok((response.status(), cache_hit, duration))
        }
        Err(err) => Err(err),
    }
}
