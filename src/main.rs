use anyhow::Result;
use clap::{App, Arg};
use log::{debug, info};
use url::Url;

mod config;
mod heater;
mod sitemaps;

fn validate_header(input: String) -> Result<(), String> {
    config::parse_header(&input).and_then(|_| Ok(()))
}

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
        .arg(
            Arg::with_name("header_variation")
                .long("header")
                .value_name("HEADER:VALUE")
                .validator(validate_header)
                .multiple(true)
                .help("header variation"),
        )
        .get_matches();

    config::Config::initialize(&matches);

    let sitemap_url = matches.value_of("sitemap_url").unwrap();

    info!("fetching sitemap from {}", sitemap_url);

    let urls: Vec<Url> = sitemaps::get(sitemap_url).await?;

    info!("... found {} URLs", urls.len());

    info!("running heater...");
    let (statuses, histogram) = heater::heat(urls.iter().cloned()).await;

    debug!("statuses: {:?}", statuses);

    info!(
        "response times: \n\tp50: {}\n\tp90: {}\n\tp99: {}",
        histogram.percentile(50.0).unwrap(),
        histogram.percentile(90.0).unwrap(),
        histogram.percentile(99.0).unwrap(),
    );

    Ok(())
}
