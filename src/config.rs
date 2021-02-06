use clap::ArgMatches;
use itertools::Itertools;
use reqwest::header::{self, HeaderMap, HeaderName, HeaderValue};
use std::collections::HashSet;
use std::convert::TryInto;
use std::iter;

pub const APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), " ", env!("CARGO_PKG_VERSION"),);

macro_rules! parse_header_tuple {
    ($header:expr,$value:expr) => {
        (
            $header.try_into().expect("unparseable header name"),
            $value.try_into().expect("unparseable header value"),
        )
    };
}

#[derive(Debug)]
pub struct Config {
    pub concurrent_requests: usize,
    header_variations: HeaderMap,
    languages: HashSet<HeaderValue>,
}

impl Config {
    pub fn new() -> Self {
        Config {
            concurrent_requests: num_cpus::get(),
            header_variations: HeaderMap::new(),
            languages: HashSet::new(),
        }
    }

    pub fn add_header_variation<TH, TV>(&mut self, header: TH, value: TV)
    where
        TH: TryInto<HeaderName>,
        TH::Error: std::fmt::Debug,
        TV: TryInto<HeaderValue>,
        TV::Error: std::fmt::Debug,
    {
        let (header, value) = parse_header_tuple!(header, value);
        self.header_variations.append(header, value);
    }

    pub fn add_language_variation<T: TryInto<HeaderValue>>(&mut self, language: T) {
        if let Ok(value) = language.try_into() {
            self.languages.insert(value);
        } else {
            panic!("could not parse language value");
        }
    }

    pub fn new_from_arguments(arguments: &ArgMatches) -> Self {
        let mut config = Self::new();

        if let Some(values) = arguments.values_of("header_variation") {
            for (h, v) in values.into_iter().filter_map(|v| parse_header(v).ok()) {
                config.add_header_variation(h, v);
            }
        }

        if let Some(values) = arguments.values_of("language") {
            for value in values {
                config.add_language_variation(value);
            }
        }

        config
    }

    pub fn possible_variations(&self) -> u64 {
        // TODO find shortcuts
        self.generate_header_variations().len() as u64
    }

    fn generate_language_variations(&self) -> Vec<HeaderValue> {
        let (empty, languages): (Vec<String>, Vec<String>) = self
            .languages
            .iter()
            .sorted()
            .dedup()
            .map(|l| l.to_str().unwrap().to_owned())
            .partition(|v| v.trim().is_empty());

        let mut response: Vec<HeaderValue> = Vec::new();
        if !(empty.is_empty()) {
            response.push(HeaderValue::from_static(""));
        }

        let len = languages.len();

        response.extend(
            // duplicate the language list x times, where x is the amount of languages
            iter::repeat(languages)
                .take(len)
                // create a cartesian product of these combinations
                .multi_cartesian_product()
                .map(|language_list| {
                    // create a joined header-value for the list of combinations
                    // language-list is made unique
                    HeaderValue::from_str(
                        &(language_list
                            .iter()
                            .unique()
                            .cloned()
                            .collect::<Vec<String>>()
                            .join(", ")),
                    )
                    .unwrap()
                })
                .collect::<Vec<HeaderValue>>(),
        );

        response
    }

    pub fn generate_header_variations(&self) -> Vec<HeaderMap> {
        if self.header_variations.is_empty() && self.languages.is_empty() {
            [HeaderMap::new()].to_vec()
        } else {
            let mut header_variations = self.header_variations.clone();

            header_variations.extend(
                self.generate_language_variations()
                    .into_iter()
                    .map(|v| (header::ACCEPT_LANGUAGE, v)),
            );

            // for every header-name, create a list of pairs (headername, value)
            // with all possible values for that header
            let v: Vec<Vec<(HeaderName, HeaderValue)>> = header_variations
                .keys()
                .cloned()
                .map(|k| {
                    header_variations
                        .get_all(&k)
                        .iter()
                        .cloned()
                        .map(|v| (k.clone(), v))
                        .collect::<Vec<(HeaderName, HeaderValue)>>()
                })
                .collect();

            // use a cartesian product to generate all possible variations
            // of these headers
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

    #[test]
    fn user_agent() {
        assert!(APP_USER_AGENT.contains("heater"));
        assert_eq!(APP_USER_AGENT.chars().filter(|&c| c == '.').count(), 2);
    }

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
        let v = cfg.generate_header_variations();
        assert_eq!(v.len(), 1);

        let headermap = &v[0];
        assert_eq!(headermap.len(), 0);
    }

    fn hm(tuples: &[(&str, &str)]) -> HeaderMap {
        tuples
            .to_vec()
            .iter()
            .cloned()
            .map(|(h, v)| parse_header_tuple!(h, v))
            .into_iter()
            .collect()
    }

    #[test]
    fn variations_two_headers_one_value() {
        let mut cfg = Config::new();
        cfg.add_header_variation("testheader", "testvalue");
        cfg.add_header_variation("testheader2", "testvalue2");

        let var = cfg.generate_header_variations();
        assert_eq!(var.len() as u64, cfg.possible_variations());

        assert_eq!(
            var[..],
            [hm(&[
                ("testheader", "testvalue"),
                ("testheader2", "testvalue2"),
            ])]
        );
    }

    #[test_case(&[""], &[""]; "empty")]
    #[test_case(&["de"], &["de"]; "de")]
    #[test_case(&["", "de"], &["", "de"]; "de + empty")]
    #[test_case(&["de", "en"], &["en", "de, en", "en, de", "de"]; "de,en")]
    #[test_case(&["", "de", "en"], &["", "en", "de, en", "en, de", "de"]; "de,en,empty")]
    fn language_variations(input: &[&str], expected: &[&str]) {
        macro_rules! hv {
            ($a:expr) => {
                HeaderValue::from_str($a).unwrap()
            };
        }

        let mut cfg = Config::new();
        for l in input {
            cfg.add_language_variation(hv!(*l));
        }

        assert_eq!(cfg.generate_language_variations().len(), expected.len());

        #[allow(clippy::mutable_key_type)]
        let v: HashSet<HeaderValue> = cfg.generate_language_variations().iter().cloned().collect();
        #[allow(clippy::mutable_key_type)]
        let expected: HashSet<HeaderValue> = expected.iter().map(|v| hv!(*v)).collect();
        assert_eq!(v, expected);

        assert_eq!(
            cfg.generate_header_variations().len() as u64,
            cfg.possible_variations()
        );

        #[allow(clippy::mutable_key_type)]
        let header_values: HashSet<HeaderValue> = cfg
            .generate_header_variations()
            .iter()
            .map(|hm| hm.get(header::ACCEPT_LANGUAGE).unwrap().clone())
            .collect();

        assert_eq!(header_values, expected);
    }

    #[test]
    fn variations_two_headers_two_values() {
        let mut cfg = Config::new();
        cfg.add_header_variation("testheader1", "testvalue1_1");
        cfg.add_header_variation("testheader1", "testvalue1_2");
        cfg.add_header_variation("testheader2", "testvalue2_1");
        cfg.add_header_variation("testheader2", "testvalue2_2");

        let var = cfg.generate_header_variations();
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
