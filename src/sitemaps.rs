use async_recursion::async_recursion;
use log::{debug, info};
use reqwest::IntoUrl;
use sitemap::{
    reader::{SiteMapEntity, SiteMapReader},
    structs::Location,
};
use thiserror::Error;
use url::Url;

#[derive(Error, Debug)]
pub enum SiteMapError {
    #[error("HTTP error")]
    RequestError(#[from] reqwest::Error),
    // #[error("data store disconnected")]
    // Disconnect(#[from] io::Error),
    // #[error("the data for key `{0}` is not available")]
    // Redaction(String),
    // #[error("invalid header (expected {expected:?}, found {found:?})")]
    // InvalidHeader {
    //     expected: String,
    //     found: String,
    // },
    // #[error("unknown data store error")]
    // Unknown,
}

#[async_recursion]
pub async fn get<T: IntoUrl + Send>(url: T) -> Result<Vec<Url>, SiteMapError> {
    let mut result: Vec<Url> = Vec::new();

    let response = reqwest::get(url).await?;

    if response.status().is_success() {
        let text = response.text().await?;
        let parser = SiteMapReader::new(text.as_bytes());
        for entity in parser {
            match entity {
                SiteMapEntity::Url(url_entry) => match url_entry.loc {
                    Location::None => {}
                    Location::Url(url) => result.push(url),
                    Location::ParseErr(err) => debug!("could not parse entry url: {:?}", err),
                },
                SiteMapEntity::SiteMap(sitemap_entry) => match sitemap_entry.loc {
                    Location::None => {}
                    Location::Url(url) => {
                        let mut urls = get(url).await?;
                        result.append(&mut urls);
                    }
                    Location::ParseErr(err) => debug!("could not parse sitemap url: {:?}", err),
                },
                SiteMapEntity::Err(_) => {
                    unimplemented!();
                }
            }
        }
    } else {
    }

    Ok(result)
}
