//! Cross-platform Rust artifact cleanup with an embedded sweep engine.

mod cli;
mod config;
mod disk;
mod history;
mod i18n;
mod safety;
mod schedule;
mod setup;
mod sweep;

fn main() {
    match cli::parse().run() {
        Ok(()) => {}
        Err(e) => {
            eprintln!("rcleaner: {e:#}");
            std::process::exit(1);
        }
    }
}
