//! Interactive setup uses the same validation and actions as the CLI.
use crate::{
    cli, config,
    i18n::{message, text, weekday},
    schedule,
};
use anyhow::Result;
use inquire::{validator::Validation, CustomType, Select, Text};
use std::path::{Path, PathBuf};

fn confirm(prompt: &'static str) -> Result<bool> {
    Ok(Select::new(text(prompt), vec![text("yes"), text("no")])
        .with_help_message(text("confirm_help"))
        .prompt()?
        == text("yes"))
}

pub fn run(root: Option<&Path>) -> Result<()> {
    println!("{}", text("wizard"));
    let (default_root, default_days) = config::resolve(root, None)?;
    let root = PathBuf::from(
        Text::new(text("ask_root"))
            .with_default(&default_root.to_string_lossy())
            .with_help_message(text("path_help"))
            .with_validator(|s: &str| {
                Ok(if Path::new(s).is_dir() {
                    Validation::Valid
                } else {
                    Validation::Invalid(message("invalid_root", &[&s]).into())
                })
            })
            .prompt()?,
    );
    let days = CustomType::<u32>::new(text("ask_days"))
        .with_default(default_days)
        .with_error_message(text("number_error"))
        .with_validator(|n: &u32| {
            Ok(if *n > 0 {
                Validation::Valid
            } else {
                Validation::Invalid(text("invalid_days").into())
            })
        })
        .prompt()?;
    let scheduled = confirm("ask_schedule")?;
    let (day, hour) = if scheduled {
        let weekdays: Vec<_> = (0..7).map(weekday).collect();
        let selected = Select::new(text("ask_weekday"), weekdays.clone())
            .with_help_message(text("select_help"))
            .prompt()?;
        let day = weekdays
            .iter()
            .position(|w| *w == selected)
            .expect("selected weekday") as u32;
        let hour = CustomType::<u32>::new(text("ask_hour"))
            .with_default(3)
            .with_error_message(text("number_error"))
            .with_validator(|n: &u32| {
                Ok(if *n <= 23 {
                    Validation::Valid
                } else {
                    Validation::Invalid(message("range_error", &[&0, &23]).into())
                })
            })
            .prompt()?;
        (day, hour)
    } else {
        (0, 3)
    };
    let run_now = confirm("ask_now")?;
    println!("{}", message("root", &[&root.display()]));
    println!("{}", message("retention", &[&days]));
    println!(
        "{}",
        message(
            "status",
            &[
                &schedule::name(),
                &text(if scheduled { "on" } else { "off" })
            ]
        )
    );
    if scheduled {
        println!("{} {hour:02}:00", weekday(day));
    }
    println!(
        "{} {}",
        text("ask_now"),
        text(if run_now { "yes" } else { "no" })
    );
    if !confirm("ask_apply")? {
        println!("{}", text("cancelled"));
        return Ok(());
    }
    if scheduled {
        cli::enable(&root, days, day, hour)?;
    } else {
        schedule::disable()?;
        config::save(&config::Config {
            root: config::validate(&root, days)?,
            days,
            cargo: config::load()?.and_then(|c| c.cargo),
        })?;
    }
    println!("{}", message("saved", &[&config::config_path().display()]));
    if run_now {
        cli::run_sweep(&root, days, false, false)?;
    }
    println!("{}", text("done"));
    Ok(())
}
