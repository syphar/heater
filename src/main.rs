use anyhow::Result;
use clap::{App, Arg};
use console::{style, Term};
use log::{debug, info};
use url::Url;

use reqwest::header;

mod config;
mod heater;
mod sitemaps;
mod status;

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
    let (statuses, cache_hits, histogram) = heater::heat(urls.iter().cloned().take(10)).await;

    if let Some(status) = status::get_progress() {
        status.finish_and_clear();
    }

    println!("{}", style("Summary").bold());

    println!("\t{}", style("Statuscodes:").bold());
    for (status, count) in statuses.iter() {
        println!("\t{:>10} => {:>5}", style(status).bold(), count);
    }

    println!("");
    println!("\t{}", style("Response times:").bold());

    for p in vec![50.0, 90.0, 99.0] {
        println!(
            "\tp{:.0}: {:>5}ms",
            style(p).bold(),
            histogram.percentile(p).unwrap()
        );
    }

    // info!("cache-hits: {:?}", cache_hits);

    Ok(())
}
