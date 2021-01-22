use async_recursion::async_recursion;
use log::warn;
use reqwest::{Client, IntoUrl};
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
}

pub async fn get<T: IntoUrl + Send>(url: T) -> Result<Vec<Url>, SiteMapError> {
    get_inner(Client::new(), url).await
}

#[async_recursion]
async fn get_inner<T: IntoUrl + Send>(client: Client, url: T) -> Result<Vec<Url>, SiteMapError> {
    let mut result: Vec<Url> = Vec::new();

    let response = client.get(url).send().await?;

    let text = response.text().await?;
    let parser = SiteMapReader::new(text.as_bytes());
    for entity in parser {
        match entity {
            SiteMapEntity::Url(url_entry) => match url_entry.loc {
                Location::None => {}
                Location::Url(url) => result.push(url),
                Location::ParseErr(err) => warn!("could not parse entry url: {:?}", err),
            },
            SiteMapEntity::SiteMap(sitemap_entry) => match sitemap_entry.loc {
                Location::None => {}
                Location::Url(url) => {
                    let mut urls = get_inner(client.clone(), url).await?;
                    result.append(&mut urls);
                }
                Location::ParseErr(err) => warn!("could not parse sitemap url: {:?}", err),
            },
            SiteMapEntity::Err(_) => {
                unimplemented!();
            }
        }
    }

    Ok(result)
}
