//! Native, per-user weekly scheduling. No shell is used for cleanup arguments.
use crate::i18n::{message, text};
use anyhow::{bail, Context, Result};
use std::{path::Path, process::Command};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
use linux as backend;
#[cfg(target_os = "macos")]
use macos as backend;
#[cfg(windows)]
use windows as backend;

pub fn name() -> &'static str {
    if cfg!(target_os = "linux") {
        "systemd"
    } else if cfg!(target_os = "macos") {
        "launchd"
    } else if cfg!(windows) {
        "Task Scheduler"
    } else {
        "unsupported"
    }
}

pub fn validate(weekday: u32, hour: u32) -> Result<()> {
    if weekday > 6 || hour > 23 {
        bail!(text("invalid_schedule"));
    }
    Ok(())
}

pub fn enable(weekday: u32, hour: u32, days: u32, root: &Path, binary: &Path) -> Result<()> {
    validate(weekday, hour)?;
    if days == 0 {
        bail!(text("invalid_days"));
    }
    let root = path_text(root)?;
    let binary = path_text(binary)?;
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    return backend::enable(weekday, hour, days, root, binary);
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    bail!(text("unsupported"))
}

pub fn disable() -> Result<()> {
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    return backend::disable();
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    bail!(text("unsupported"))
}

pub fn is_loaded() -> Result<bool> {
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    return backend::is_loaded();
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    bail!(text("unsupported"))
}

/// Resolve once before any scheduler mutation; never fall back to the working directory.
#[cfg(any(target_os = "macos", test))]
fn required_home(home: Option<std::path::PathBuf>) -> Result<std::path::PathBuf> {
    let home = home
        .filter(|path| path.is_absolute())
        .context(text("missing_home"))?;
    path_text(&home)?;
    Ok(home)
}

fn path_text(path: &Path) -> Result<&str> {
    let value = path.to_str().context(text("invalid_path"))?;
    if value.chars().any(char::is_control) {
        bail!(text("invalid_path"));
    }
    Ok(value)
}

fn checked(command: &mut Command) -> Result<std::process::Output> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command
        .output()
        .with_context(|| message("exec_failed", &[&program]))?;
    if !output.status.success() {
        bail!(message(
            "command_failed",
            &[
                &program,
                &format!(
                    "{}{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                )
            ]
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_or_relative_home_is_rejected() {
        assert!(required_home(None).is_err());
        assert!(required_home(Some(std::path::PathBuf::from("."))).is_err());
        let temp = tempfile::tempdir().unwrap();
        assert_eq!(
            required_home(Some(temp.path().to_path_buf())).unwrap(),
            temp.path()
        );
    }
    #[test]
    fn scheduler_rejects_control_characters() {
        assert!(path_text(Path::new("/tmp/a\nExecStart=bad")).is_err());
        assert!(path_text(Path::new("/tmp/a & b")).is_ok());
    }
}
