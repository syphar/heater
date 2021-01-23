use clap::ArgMatches;
use once_cell::sync::OnceCell;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Config {
    pub concurrent_requests: usize,
    pub header_variations: HashMap<String, Vec<String>>,
}

static CONFIG: OnceCell<Config> = OnceCell::new();

impl Config {
    pub fn initialize(arguments: &ArgMatches) {
        // TODO: arguments for header variations
        CONFIG
            .set(Config {
                concurrent_requests: num_cpus::get(),
                header_variations: HashMap::new(),
            })
            .unwrap();
    }

    pub fn get() -> &'static Config {
        CONFIG.get().expect("config is not initialized")
    }
}
