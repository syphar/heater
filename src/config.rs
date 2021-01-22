use once_cell::sync::Lazy;

pub static CONCURRENT_REQUESTS: Lazy<usize> = Lazy::new(|| num_cpus::get());
