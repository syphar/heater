use clap::ArgMatches;
use itertools::Itertools;
use once_cell::sync::OnceCell;
use reqwest::header::{self, HeaderMap, HeaderName, HeaderValue};
use std::convert::TryInto;

#[derive(Debug)]
pub struct Config {
    pub concurrent_requests: usize,
    header_variations: header::HeaderMap,
}

static CONFIG: OnceCell<Config> = OnceCell::new();

impl Config {
    fn new() -> Self {
        Config {
            concurrent_requests: num_cpus::get(),
            header_variations: HeaderMap::new(),
        }
    }

    fn header_tuple<TH: TryInto<HeaderName>, TV: TryInto<HeaderValue>>(
        header: TH,
        value: TV,
    ) -> (HeaderName, HeaderValue) {
        (
            if let Ok(header) = header.try_into() {
                header
            } else {
                panic!("could not parse header");
            },
            if let Ok(value) = value.try_into() {
                value
            } else {
                panic!("could not parse header value");
            },
        )
    }

    pub fn add_header_variation<TH: TryInto<HeaderName>, TV: TryInto<HeaderValue>>(
        &mut self,
        header: TH,
        value: TV,
    ) {
        let (header, value) = Self::header_tuple(header, value);
        self.header_variations.append(header, value);
    }

    pub fn initialize(arguments: &ArgMatches) {
        let mut config = Self::new();

        if let Some(values) = arguments.values_of("header_variation") {
            for (h, v) in values.into_iter().filter_map(|v| parse_header(v).ok()) {
                config.add_header_variation(h, v);
            }
        }

        CONFIG.set(config).unwrap();
    }

    pub fn initialize_empty() {
        let _ = CONFIG.set(Config::new());
    }

    pub fn get() -> &'static Config {
        CONFIG.get().expect("config is not initialized")
    }

    pub fn possible_variations(&self) -> u64 {
        if self.header_variations.is_empty() {
            1
        } else {
            self.header_variations
                .keys()
                .map(|k| self.header_variations.get_all(k).iter().count())
                .product::<usize>() as u64
        }
    }

    pub fn header_variations(&self) -> Vec<HeaderMap> {
        if self.header_variations.is_empty() {
            [HeaderMap::new()].to_vec()
        } else {
            // for every header-name, create a list of pairs (headername, value)
            let v: Vec<Vec<(HeaderName, HeaderValue)>> = self
                .header_variations
                .keys()
                .cloned()
                .map(|k| {
                    self.header_variations
                        .get_all(&k)
                        .iter()
                        .cloned()
                        .map(|v| (k.clone(), v))
                        .collect::<Vec<(HeaderName, HeaderValue)>>()
                })
                .collect();

            // use a cartesian product to return the values
            v.iter()
                .cloned()
                .multi_cartesian_product()
                .map(|o| o.iter().cloned().collect::<HeaderMap>())
                .collect()
        }
    }
}

pub fn parse_header(input: &str) -> Result<(header::HeaderName, header::HeaderValue), String> {
    let mut s = input.splitn(2, ':');

    let header = if let Some(hn) = s.next() {
        if let Ok(header) = hn.parse::<header::HeaderName>() {
            header
        } else {
            return Err(format!("could not parse header: {}", hn));
        }
    } else {
        return Err("missing separator ':' in header definition".to_string());
    };

    let value = if let Some(val) = s.next() {
        if let Ok(value) = val.parse::<header::HeaderValue>() {
            value
        } else {
            return Err(format!("invalid header value: {}", val));
        }
    } else {
        return Err("could not find value".to_string());
    };

    Ok((header, value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderName, HeaderValue};
    use test_case::test_case;

    #[test_case(""; "empty")]
    #[test_case(":"; "only colon")]
    #[test_case("a test with space:value "; "spaces")]
    fn header_validation_err(text: &str) {
        assert!(parse_header(text).is_err());
    }

    #[test_case(
        "accept-language:value",
        HeaderName::from_static("accept-language"),
        HeaderValue::from_static("value")
    )]
    #[test_case(
        "accept-language:value with space",
        HeaderName::from_static("accept-language"),
        HeaderValue::from_static("value with space")
    )]
    #[test_case(
        "accept-language-empty:",
        HeaderName::from_static("accept-language-empty"),
        HeaderValue::from_static("")
    )]
    fn header_validation_ok(text: &str, header: HeaderName, value: HeaderValue) {
        assert_eq!(parse_header(text), Ok((header, value)));
    }

    #[test]
    fn variations_empty() {
        let cfg = Config::new();
        assert_eq!(cfg.possible_variations(), 1);
        let v = cfg.header_variations();
        assert_eq!(v.len(), 1);

        let headermap = &v[0];
        assert_eq!(headermap.len(), 0);
    }

    fn hm(tuples: &[(&str, &str)]) -> HeaderMap {
        tuples
            .to_vec()
            .iter()
            .cloned()
            .map(|(h, v)| Config::header_tuple(h, v))
            .into_iter()
            .collect()
    }

    #[test]
    fn variations_two_headers_one_value() {
        let mut cfg = Config::new();
        cfg.add_header_variation("testheader", "testvalue");
        cfg.add_header_variation("testheader2", "testvalue2");

        let var = cfg.header_variations();
        assert_eq!(var.len() as u64, cfg.possible_variations());

        assert_eq!(
            var[..],
            [hm(&[
                ("testheader", "testvalue"),
                ("testheader2", "testvalue2"),
            ])]
        );
    }

    #[test]
    fn variations_two_headers_two_values() {
        let mut cfg = Config::new();
        cfg.add_header_variation("testheader1", "testvalue1_1");
        cfg.add_header_variation("testheader1", "testvalue1_2");
        cfg.add_header_variation("testheader2", "testvalue2_1");
        cfg.add_header_variation("testheader2", "testvalue2_2");

        let var = cfg.header_variations();
        assert_eq!(var.len() as u64, cfg.possible_variations());

        let expected = [
            hm(&[
                ("testheader1", "testvalue1_1"),
                ("testheader2", "testvalue2_1"),
            ]),
            hm(&[
                ("testheader1", "testvalue1_1"),
                ("testheader2", "testvalue2_2"),
            ]),
            hm(&[
                ("testheader1", "testvalue1_2"),
                ("testheader2", "testvalue2_1"),
            ]),
            hm(&[
                ("testheader1", "testvalue1_2"),
                ("testheader2", "testvalue2_2"),
            ]),
        ];

        for (i, hm) in var.iter().enumerate() {
            assert_eq!(expected[i], *hm);
        }
    }
}
