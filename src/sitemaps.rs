use async_recursion::async_recursion;
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

    #[error("XML parsing error")]
    XmlError(#[from] xml::reader::Error),
}

pub async fn get<T: IntoUrl + Send>(url: T) -> Result<Vec<Url>, SiteMapError> {
    get_inner(Client::new(), url).await
}

#[async_recursion]
async fn get_inner<T>(client: Client, url: T) -> Result<Vec<Url>, SiteMapError>
where
    T: IntoUrl + Send,
{
    let mut result: Vec<Url> = Vec::new();

    let response = client.get(url).send().await?;

    let text = response.text().await?;
    let parser = SiteMapReader::new(text.as_bytes());
    for entity in parser {
        match entity {
            SiteMapEntity::Url(url_entry) => match url_entry.loc {
                Location::None => {}
                Location::Url(url) => result.push(url),
                Location::ParseErr(err) => log::warn!("could not parse entry url: {err:?}"),
            },
            SiteMapEntity::SiteMap(sitemap_entry) => match sitemap_entry.loc {
                Location::None => {}
                Location::Url(url) => {
                    let mut urls = get_inner(client.clone(), url).await?;
                    result.append(&mut urls);
                }
                Location::ParseErr(err) => log::warn!("could not parse sitemap url: {err:?}"),
            },
            SiteMapEntity::Err(err) => return Err(SiteMapError::XmlError(err)),
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn does_not_exist() {
        assert!(get("http://does/not/exist").await.is_err());
    }

    #[tokio::test]
    async fn invalid_xml() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/sitemap.xml")
            .with_status(200)
            .with_header("content-type", "text/xml")
            .with_body("asdf")
            .create_async()
            .await;

        assert!(get(&format!("{}/sitemap.xml", server.url())).await.is_err());
    }

    #[tokio::test]
    async fn load_single_sitemap() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/sitemap.xml")
            .with_status(200)
            .with_header("content-type", "text/xml")
            .with_body(
                r#"
              <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                <url>
                    <loc>http://www.example.com/</loc>
                </url>
            </urlset>"#,
            )
            .create_async()
            .await;

        assert_eq!(
            get(&format!("{}/sitemap.xml", server.url())).await.unwrap()[..],
            [Url::parse("http://www.example.com/").unwrap()],
        );
    }

    #[tokio::test]
    async fn load_sub_sitemaps() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/sitemap.xml")
            .with_status(200)
            .with_header("content-type", "text/xml")
            .with_body(format!(
                r#"
                <sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                    <sitemap>
                        <loc>{}/real_sitemap.xml</loc>
                    </sitemap>
                </sitemapindex>"#,
                server.url()
            ))
            .create_async()
            .await;

        let _i = server
            .mock("GET", "/real_sitemap.xml")
            .with_status(200)
            .with_header("content-type", "text/xml")
            .with_body(
                r#"
                <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                    <url>
                        <loc>http://www.example11.com/</loc>
                    </url>
                </urlset>"#,
            )
            .create_async()
            .await;

        assert_eq!(
            get(&format!("{}/sitemap.xml", server.url())).await.unwrap()[..],
            [Url::parse("http://www.example11.com/").unwrap()],
        );
    }
}
