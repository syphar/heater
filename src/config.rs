use clap::ArgMatches;
use once_cell::sync::OnceCell;
use reqwest::header;
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub struct Config {
    pub concurrent_requests: usize,
    pub header_variations: HashMap<header::HeaderName, HashSet<header::HeaderValue>>,
}

static CONFIG: OnceCell<Config> = OnceCell::new();

impl Config {
    pub fn initialize(arguments: &ArgMatches) {
        let mut variations: HashMap<header::HeaderName, HashSet<header::HeaderValue>> =
            HashMap::new();
        if let Some(values) = arguments.values_of("header_variation") {
            for (header, value) in values.into_iter().filter_map(|v| parse_header(v).ok()) {
                if !(variations.contains_key(&header)) {
                    variations.insert(header.clone(), HashSet::new());
                }
                variations.get_mut(&header).unwrap().insert(value);
            }
        }

        log::debug!("loaded variations: {:?}", variations);

        CONFIG
            .set(Config {
                concurrent_requests: num_cpus::get(),
                header_variations: variations,
            })
            .unwrap();
    }

    pub fn get() -> &'static Config {
        CONFIG.get().expect("config is not initialized")
    }
}

pub fn parse_header(input: &str) -> Result<(header::HeaderName, header::HeaderValue), String> {
    let mut s = input.splitn(2, ":");

    let header = if let Some(hn) = s.next() {
        if hn.is_empty() {
            return Err("Empty header".to_string());
        } else {
            if let Ok(header) = hn.parse::<header::HeaderName>() {
                header
            } else {
                return Err(format!("could not parse header: {}", hn));
            }
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
}
