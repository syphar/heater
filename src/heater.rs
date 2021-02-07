use crate::{config::Config, status};
use counter::Counter;
use futures::{stream, StreamExt};
use histogram::Histogram;
use itertools::iproduct;
use reqwest::{
    header::{self, HeaderMap, HeaderName},
    Client, IntoUrl, StatusCode,
};
use std::time::{Duration, Instant};

pub async fn heat<T: 'static + IntoUrl + Send + Clone>(
    config: &Config,
    urls: impl Iterator<Item = T>,
) -> (Counter<StatusCode>, Counter<Option<bool>>, Histogram) {
    let client = Client::builder().gzip(true).build().unwrap();

    stream::iter(iproduct!(
        urls,
        config.generate_header_variations()
    ))
    .map(|(url, hm)| {
        let client = client.clone();
        let hm = hm.clone();
        tokio::spawn(async move { heat_one(&client, url, hm).await })
    })
    .buffer_unordered(config.concurrent_requests)
    .map(|result| {
        result // while tokio join errors should always panic,
            .unwrap_or_else(|err| panic!("tokio error: {:?}", err))
            // TODO: reqwest errors should be handled differently
            .unwrap_or_else(|err| panic!("reqwest error error: {:?}", err))
    })
    .fold(
        (Counter::new(), Counter::new(), Histogram::new()),
        |(mut acc_status, mut acc_cache, mut histogram), (status, cache_hit, elapsed)| async move {
            acc_status[&status] += 1;
            acc_cache[&cache_hit] += 1;
            histogram.increment(elapsed.as_millis() as u64).unwrap();

            (acc_status, acc_cache, histogram)
        },
    )
    .await
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

    let result = match request.send().await {
        Ok(response) => {
            let duration = start.elapsed();

            // log a warning if the `Vary` header contains of values which
            // are not defined in the header variations.

            if log::max_level() >= log::LevelFilter::Warn {
                for value in response
                    .headers()
                    .get_all(header::VARY)
                    .iter()
                    .filter_map(|v| v.to_str().ok())
                {
                    for header_name in value
                        .split(',')
                        .map(|v| v.trim())
                        .filter_map(|s| s.parse::<HeaderName>().ok())
                    {
                        if !(headers.contains_key(&header_name)) {
                            log::warn!("received Vary header '{}' that is missing in configured header variations", header_name);
                        }
                    }
                }
            }

            let cache_hit = response
                .headers()
                .get(HeaderName::from_static("x-cache"))
                .map(|value| value.to_str().unwrap_or("")[0..3].to_lowercase() == "hit");

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
    use crate::config;
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
            .match_header(header::ACCEPT_ENCODING.as_ref(), "gzip")
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
