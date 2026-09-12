//! Command-line interface shared by all supported platforms.
use crate::{
    config, history,
    i18n::{message, text, weekday},
    schedule, sweep,
};
use anyhow::Result;
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "rcleaner", version, about = text("about"))]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[arg(long, short, global = true, help = text("root_help"))]
    root: Option<PathBuf>,
}

#[derive(Subcommand)]
pub enum Command {
    #[command(about = text("setup_help"))]
    Setup,
    #[command(about = text("sweep_help"))]
    Sweep {
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..), help = text("days_help"))]
        days: Option<u32>,
        #[arg(long, help = text("dry_help"))]
        dry_run: bool,
        #[arg(long, help = text("force_help"))]
        force: bool,
    },
    #[command(about = text("enable_help"))]
    Enable {
        #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u32).range(0..=6), help = text("weekday_help"))]
        weekday: u32,
        #[arg(long, default_value_t = 3, value_parser = clap::value_parser!(u32).range(0..=23), help = text("hour_help"))]
        hour: u32,
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..), help = text("days_help"))]
        days: Option<u32>,
    },
    #[command(about = text("disable_help"))]
    Disable,
    #[command(about = text("status_help"))]
    Status,
    #[command(about = text("history_help"))]
    History {
        #[arg(long, short, default_value_t = 10, help = text("limit_help"))]
        limit: usize,
    },
}

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command {
            None => {
                let (root, days) = config::resolve(self.root.as_deref(), None)?;
                run_sweep(&root, days, false, false)
            }
            Some(Command::Setup) => crate::setup::run(self.root.as_deref()),
            Some(Command::Sweep {
                days,
                dry_run,
                force,
            }) => {
                let (root, days) = config::resolve(self.root.as_deref(), days)?;
                run_sweep(&root, days, dry_run, force)
            }
            Some(Command::Enable {
                weekday: day,
                hour,
                days,
            }) => {
                let (root, days) = config::resolve(self.root.as_deref(), days)?;
                enable(&root, days, day, hour)
            }
            Some(Command::Disable) => {
                schedule::disable()?;
                println!("{}", text("disabled"));
                Ok(())
            }
            Some(Command::Status) => {
                println!(
                    "{}",
                    message(
                        "status",
                        &[
                            &schedule::name(),
                            &text(if schedule::is_loaded()? { "on" } else { "off" })
                        ]
                    )
                );
                if let Some(last) = history::read(1)?.first() {
                    println!("{}", message("last", &[&last.timestamp]));
                    print_report(last);
                } else {
                    println!("{}", text("no_history"));
                }
                Ok(())
            }
            Some(Command::History { limit }) => {
                let records = history::read(limit)?;
                if records.is_empty() {
                    println!("{}", text("no_history"));
                    return Ok(());
                }
                println!("{}", text("history_header"));
                for r in records {
                    println!(
                        "{}  {}  {}  {} → {}",
                        fmt_ts(&r.timestamp),
                        text(if r.mode == "dry-run" {
                            "dry-run"
                        } else {
                            "live"
                        }),
                        r.total_freed.as_deref().unwrap_or("—"),
                        r.disk_before,
                        r.disk_after
                    );
                    if r.skipped {
                        println!(
                            "{}",
                            message(
                                "skipped",
                                &[&r.skip_reason.as_deref().unwrap_or(text("unknown"))]
                            )
                        );
                    }
                }
                Ok(())
            }
        }
    }
}

pub(crate) fn enable(root: &Path, days: u32, day: u32, hour: u32) -> Result<()> {
    let root = config::validate(root, days)?;
    schedule::validate(day, hour)?;
    let cargo = Some(sweep::find_cargo()?);
    let binary = install_self_binary()?;
    config::save(&config::Config {
        root: root.clone(),
        days,
        cargo,
    })?;
    schedule::enable(day, hour, days, &root, &binary)?;
    println!(
        "{}",
        message(
            "enabled",
            &[&schedule::name(), &weekday(day), &format!("{hour:02}")]
        )
    );
    println!("{}", message("root", &[&root.display()]));
    println!("{}", message("retention", &[&days]));
    println!("{}", message("binary", &[&binary.display()]));
    Ok(())
}

pub(crate) fn run_sweep(root: &Path, days: u32, dry_run: bool, force: bool) -> Result<()> {
    eprintln!(
        "{}",
        message(
            "run",
            &[
                &root.display(),
                &days,
                &text(if dry_run { "dry-run" } else { "live" })
            ]
        )
    );
    let report = sweep::run(root, days, dry_run, force)?;
    print_report(&report);
    if !dry_run {
        history::append(&report)?;
    }
    Ok(())
}

fn print_report(report: &sweep::SweepReport) {
    if report.skipped {
        println!(
            "{}",
            message(
                "skipped",
                &[&report.skip_reason.as_deref().unwrap_or(text("unknown"))]
            )
        );
        if report.projects.is_empty() {
            return;
        }
    }
    for project in &report.projects {
        println!(
            "  {:>10}  {}",
            project.freed.as_deref().unwrap_or("—"),
            project.path
        );
    }
    if let Some(total) = report
        .total_freed
        .as_deref()
        .filter(|total| *total != "0.00 B")
    {
        println!(
            "{}",
            message("total", &[&total, &report.disk_before, &report.disk_after])
        );
    } else {
        println!("{}", message("nothing", &[&report.disk_before]));
    }
}

fn fmt_ts(ts: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(ts)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|_| ts.to_string())
}

/// Keep the scheduled executable outside build artifacts, including on Windows.
pub(crate) fn install_self_binary() -> Result<PathBuf> {
    let src = std::env::current_exe()?.canonicalize()?;
    let dest = config::config_dir().join(format!("rcleaner{}", std::env::consts::EXE_SUFFIX));
    std::fs::create_dir_all(config::config_dir())?;
    install_binary(&src, &dest)?;
    Ok(dest)
}

fn install_binary(src: &Path, dest: &Path) -> Result<()> {
    let metadata = match std::fs::symlink_metadata(dest) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    if metadata
        .as_ref()
        .is_some_and(|m| !m.file_type().is_symlink())
        && dest.canonicalize()? == src
    {
        return Ok(());
    }
    // Stage first so failed copies do not truncate the installed executable.
    let staging = dest.with_extension("new");
    std::fs::copy(src, &staging)?;
    replace_staged_binary(&staging, dest)
}

/// Replace in place without deleting the installed binary before the commit step.
fn replace_staged_binary(staging: &Path, dest: &Path) -> Result<()> {
    // The sibling staging file is on the same filesystem. rename replaces an
    // existing file on Windows too; a failed rename must leave it available.
    std::fs::rename(staging, dest)?;
    Ok(())
}

/// Localize generated help as well as application descriptions.
pub fn parse() -> Cli {
    fn localized(mut command: clap::Command) -> clap::Command {
        command = command.disable_help_subcommand(true);
        command.build();
        command
            .help_template(text("help_template"))
            .subcommand_help_heading(text("commands"))
            .mut_args(|arg| {
                let arg = match arg.get_id().as_str() {
                    "help" => arg.help(text("help_flag")),
                    "version" => arg.help(text("version_flag")),
                    _ => arg,
                };
                arg.help_heading(text("options"))
            })
            .mut_subcommands(localized)
    }
    let mut command = localized(Cli::command());
    let matches = command.clone().get_matches();
    Cli::from_arg_matches(&matches).unwrap_or_else(|error| error.format(&mut command).exit())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failed_replacement_preserves_installed_binary() {
        let temp = tempfile::tempdir().unwrap();
        let dest = temp.path().join("rcleaner");
        std::fs::write(&dest, "installed version").unwrap();
        // Model a staged file disappearing before the final rename.
        let error = replace_staged_binary(&temp.path().join("missing.new"), &dest).unwrap_err();
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::NotFound
        );
        assert_eq!(std::fs::read_to_string(dest).unwrap(), "installed version");
    }

    #[test]
    fn installation_replaces_existing_regular_file() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let dest = temp.path().join("rcleaner");
        std::fs::write(&source, "new version").unwrap();
        std::fs::write(&dest, "old version").unwrap();
        install_binary(&source.canonicalize().unwrap(), &dest).unwrap();
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "new version");
        assert!(!dest.with_extension("new").exists());
    }

    #[cfg(unix)]
    #[test]
    fn installation_replaces_symlink_to_running_binary() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("target/rcleaner");
        std::fs::create_dir(source.parent().unwrap()).unwrap();
        std::fs::write(&source, "binary").unwrap();
        let source = source.canonicalize().unwrap();
        let dest = temp.path().join("rcleaner");
        std::os::unix::fs::symlink(&source, &dest).unwrap();
        install_binary(&source, &dest).unwrap();
        assert!(std::fs::symlink_metadata(&dest)
            .unwrap()
            .file_type()
            .is_file());
        std::fs::remove_file(source).unwrap();
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "binary");
        install_binary(&dest.canonicalize().unwrap(), &dest).unwrap();
        assert_eq!(std::fs::read_to_string(dest).unwrap(), "binary");
    }

    #[test]
    fn omitted_days_uses_config() {
        let cli = Cli::try_parse_from(["rcleaner", "sweep"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Sweep { days: None, .. })
        ));
    }
    #[test]
    fn rejects_invalid_schedule_and_retention() {
        for args in [
            ["rcleaner", "enable", "--weekday", "7"],
            ["rcleaner", "enable", "--hour", "24"],
            ["rcleaner", "sweep", "--days", "0"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
    }
    #[test]
    fn timestamps_preserve_offset() {
        assert_eq!(fmt_ts("2026-06-15T16:11:43+09:00"), "2026-06-15 16:11");
        assert_eq!(fmt_ts("invalid"), "invalid");
    }
}
