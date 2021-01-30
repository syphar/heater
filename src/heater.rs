use crate::{
    config::{self, Config},
    status,
};
use counter::Counter;
use futures::{stream, StreamExt};
use histogram::Histogram;
use reqwest::{
    header::{self, HeaderMap, HeaderName},
    Client, IntoUrl, StatusCode,
};
use std::collections::HashSet;
use std::time::{Duration, Instant};

pub async fn heat<T: 'static + IntoUrl + Send + Clone>(
    config: &Config,
    urls: impl Iterator<Item = T>,
) -> (Counter<StatusCode>, Counter<Option<bool>>, Histogram) {
    let header_variations = config.header_variations();

    let client = Client::builder()
        .user_agent(config::APP_USER_AGENT)
        .gzip(true)
        .build()
        .unwrap();

    let todo = urls
        .map(|url| {
            header_variations
                .iter()
                .map(|hm| (url.clone(), hm))
                .collect::<Vec<_>>()
        })
        .flatten();

    let stats: Vec<_> = stream::iter(todo)
        .map(|(url, hm)| {
            let client = client.clone();
            let hm = hm.clone();
            tokio::spawn(async move { heat_one(&client, url, hm).await })
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
    headers: HeaderMap,
) -> Result<(StatusCode, Option<bool>, Duration), reqwest::Error> {
    let start = Instant::now();

    let mut request = client.get(url);
    for (h, v) in headers.iter() {
        request = request.header(h, v);
    }

    #[allow(clippy::mutable_key_type)]
    let configured_headers: HashSet<HeaderName> = headers.keys().cloned().collect();

    let result = match request.send().await {
        Ok(response) => {
            let duration = start.elapsed();

            // log a warning if the `Vary` header contains of values which
            // are not defined in the header variations.
            #[allow(clippy::mutable_key_type)]
            for headervalue in response.headers().get_all(header::VARY) {
                if let Ok(value) = headervalue.to_str() {
                    let headers_in_request: HashSet<HeaderName> = value
                        .split(',')
                        .map(|v| v.trim())
                        .map(|s| s.parse())
                        .filter_map(Result::ok)
                        .collect();

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
            Ok((response.status(), cache_hit, duration))
        }
        Err(err) => Err(err),
    };

    if let Some(st) = status::get_progress() {
        st.inc(1);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{self, mock};
    use reqwest::{self, Url};
    use test_case::test_case;

    #[tokio::test]
    async fn empty_list() {
        let config = Config::new();
        let urls: Vec<Url> = Vec::new();
        heat(&config, urls.iter().cloned()).await;
    }

    #[tokio::test]
    async fn heat_single_page_simple() {
        let m = mock("GET", "/dummy.xml").with_status(200).create();

        let urls: Vec<Url> =
            vec![Url::parse(&format!("{}/dummy.xml", mockito::server_url())).unwrap()];

        let config = Config::new();
        let (statuses, cdn, stats) = heat(&config, urls.iter().cloned()).await;

        m.assert();

        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses.get(&StatusCode::OK), Some(&1));
        assert_eq!(stats.entries(), 1);
        assert_eq!(cdn.len(), 1);
        assert_eq!(cdn.get(&None), Some(&1));
    }

    #[test_case("HIT", true)]
    #[test_case("MISS", false)]
    #[tokio::test]
    async fn heat_single_page_cdn(header_value: &str, expected: bool) {
        let m = mock("GET", "/dummy.xml")
            .with_status(200)
            .with_header("x-cache", header_value)
            .create();

        let urls: Vec<Url> =
            vec![Url::parse(&format!("{}/dummy.xml", mockito::server_url())).unwrap()];

        let config = Config::new();
        let (statuses, cdn, stats) = heat(&config, urls.iter().cloned()).await;

        m.assert();

        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses.get(&StatusCode::OK), Some(&1));
        assert_eq!(stats.entries(), 1);
        assert_eq!(cdn.len(), 1);
        assert_eq!(cdn.get(&Some(expected)), Some(&1));
    }

    #[tokio::test]
    async fn heat_single_page_with_headers() {
        #[allow(clippy::borrow_interior_mutable_const)]
        let m = mock("GET", "/dummy.xml")
            .match_header("dummyheader", "dummyvalue")
            .match_header(header::ACCEPT_ENCODING.as_ref(), "gzip, br")
            .match_header(header::USER_AGENT.as_ref(), config::APP_USER_AGENT)
            .with_status(200)
            .with_body("test")
            .create();

        let mut config = Config::new();
        config.add_header_variation("dummyheader", "dummyvalue");

        let urls: Vec<Url> =
            vec![Url::parse(&format!("{}/dummy.xml", mockito::server_url())).unwrap()];

        let (statuses, cdn, stats) = heat(&config, urls.iter().cloned()).await;

        m.assert();

        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses.get(&StatusCode::OK), Some(&1));
        assert_eq!(stats.entries(), 1);
        assert_eq!(cdn.len(), 1);
        assert_eq!(cdn.get(&None), Some(&1));
    }
}
