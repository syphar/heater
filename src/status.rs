use indicatif::ProgressBar;
use once_cell::sync::OnceCell;

static PROGRESS: OnceCell<ProgressBar> = OnceCell::new();

pub fn initialize_progress(len: u64) {
    PROGRESS.set(ProgressBar::new(len)).unwrap();
}

pub fn get_progress() -> Option<&'static ProgressBar> {
    PROGRESS.get()
}
