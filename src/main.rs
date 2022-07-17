use anyhow::Result;
use clap::{crate_authors, crate_name, crate_version, App, Arg};
use console::style;
use log::info;
use url::Url;

mod config;
mod heater;
mod sitemaps;
mod status;

fn validate_header(input: &str) -> Result<(), String> {
    config::parse_header(input)
        .map(|_| ())
        .map_err(|e| format!("{}", e))
}

#[tokio::main]
pub async fn main() -> Result<()> {
    pretty_env_logger::init();

    let matches = App::new(crate_name!())
        .about("heats up website caches")
        .version(crate_version!())
        .author(crate_authors!())
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
        .arg(
            Arg::with_name("language")
                .long("language")
                .value_name("IEFT language tag")
                .multiple(true)
                .help(
                    "language tags will be used to generate all \
                    possible permutations of these languages, \
                    including their order",
                ),
        )
        .get_matches();

    let config = config::Config::new_from_arguments(&matches);

    let sitemap_url = matches.value_of("sitemap_url").unwrap();

    info!("fetching sitemap from {}", sitemap_url);

    let urls: Vec<Url> = sitemaps::get(sitemap_url).await?;

    info!("... found {} URLs", urls.len());
    status::initialize_progress(urls.len() as u64 * config.possible_variations());

    info!("running heater...");
    let (statuses, cache_hits, histogram) = heater::heat(&config, urls.iter().cloned()).await;

    if let Some(status) = status::get_progress() {
        status.finish_and_clear();
    }

    println!("{}", style("Summary").bold());

    println!("\t{}", style("Statuscodes:").bold());
    for (status, count) in statuses.iter() {
        println!("\t{:>10} => {:>5}", style(status).bold(), count);
    }

    println!();
    println!("\t{}", style("Response times:").bold());
    for p in &[50.0, 90.0, 99.0] {
        println!(
            "\tp{:.0}: {:>5}ms",
            style(p).bold(),
            histogram.percentile(*p).unwrap()
        );
    }

    if cache_hits.keys().any(|h| h.is_some()) {
        println!();
        println!("\t{}", style("CDN caching:").bold());

        if let Some(h) = cache_hits.get(&Some(true)) {
            println!("\t{:>4}: {:>7}", style("HIT").bold(), h);
        }
        if let Some(h) = cache_hits.get(&Some(false)) {
            println!("\t{:>4}: {:>7}", style("MISS").bold(), h);
        }
        if let Some(h) = cache_hits.get(&None) {
            println!("\t{}: {:>7}", style("UNKNOWN").italic(), h);
        }
    }

    Ok(())
}
