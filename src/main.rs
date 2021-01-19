use anyhow::Result;
use clap::{App, Arg};
use log::info;
use url::Url;

mod sitemaps;

#[tokio::main]
pub async fn main() -> Result<()> {
    pretty_env_logger::init();

    let matches = App::new("heater")
        .about("heats up website caches")
        .arg(
            Arg::with_name("sitemap_url")
                .help("sitemap URL")
                .required(true)
                .index(1),
        )
        .get_matches();

    let sitemap_url = matches.value_of("sitemap_url").unwrap();

    info!("fetching sitemap from {}", sitemap_url);

    let urls: Vec<Url> = sitemaps::get(sitemap_url).await?;

    info!("... found {} URLs", urls.len());

    Ok(())
}
