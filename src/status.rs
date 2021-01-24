use indicatif::{ProgressBar, ProgressStyle};
use once_cell::sync::OnceCell;

static PROGRESS: OnceCell<ProgressBar> = OnceCell::new();

pub fn initialize_progress(len: u64) {
    let bar = ProgressBar::new(len);
    bar.set_style(
        ProgressStyle::default_bar().template("[ETA: {eta_precise}] {wide_bar} {pos}/{len}"),
    );

    let _ = PROGRESS.set(bar);
}

pub fn get_progress() -> Option<&'static ProgressBar> {
    PROGRESS.get()
}
