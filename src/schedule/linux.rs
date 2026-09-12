use super::checked;
use anyhow::Result;
use std::{fs, path::PathBuf, process::Command};

const TIMER: &str = "rcleaner.timer";
const SERVICE: &str = "rcleaner.service";
fn unit_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(crate::config::config_dir)
        .join("systemd/user")
}

// systemd has its own quoting and expansion rules, independent of shell quoting.
fn quote(value: &str) -> String {
    format!(
        "\"{}\"",
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
    )
}

fn service(days: u32, root: &str, binary: &str) -> String {
    format!("[Unit]\nDescription=Rcleaner Rust build artifact cleanup\n\n[Service]\nType=oneshot\nExecStart=:{} sweep --days {days} --root {}\n", quote(binary), quote(root))
}
fn timer(weekday: u32, hour: u32) -> String {
    let day = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][weekday as usize];
    format!("[Unit]\nDescription=Weekly Rcleaner cleanup\n\n[Timer]\nOnCalendar={day} *-*-* {hour:02}:00:00\nPersistent=true\nUnit={SERVICE}\n\n[Install]\nWantedBy=timers.target\n")
}

pub fn enable(weekday: u32, hour: u32, days: u32, root: &str, binary: &str) -> Result<()> {
    // Probe the user manager before writing anything; systemd is optional for manual use.
    checked(Command::new("systemctl").args(["--user", "show-environment"]))?;
    fs::create_dir_all(unit_dir())?;
    fs::write(unit_dir().join(SERVICE), service(days, root, binary))?;
    fs::write(unit_dir().join(TIMER), timer(weekday, hour))?;
    checked(Command::new("systemctl").args(["--user", "daemon-reload"]))?;
    checked(Command::new("systemctl").args(["--user", "enable", TIMER]))?;
    checked(Command::new("systemctl").args(["--user", "restart", TIMER]))?;
    Ok(())
}

pub fn disable() -> Result<()> {
    if !unit_dir().join(TIMER).exists() && !unit_dir().join(SERVICE).exists() {
        return Ok(());
    }
    checked(Command::new("systemctl").args(["--user", "disable", "--now", TIMER]))?;
    for name in [TIMER, SERVICE] {
        let path = unit_dir().join(name);
        if path.exists() {
            fs::remove_file(path)?;
        }
    }
    checked(Command::new("systemctl").args(["--user", "daemon-reload"]))?;
    Ok(())
}

pub fn is_loaded() -> Result<bool> {
    if !unit_dir().join(TIMER).exists() {
        return Ok(false);
    }
    let output = checked(Command::new("systemctl").args([
        "--user",
        "show",
        TIMER,
        "--property=ActiveState",
        "--value",
    ]))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim() == "active")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_are_single_arguments_without_expansion() {
        assert_eq!(
            quote("/home/a b/%x/$HOME/\"q\"/\\"),
            "\"/home/a b/%%x/$HOME/\\\"q\\\"/\\\\\""
        );
        let unit = service(45, "/tmp/Проекты & x", "/tmp/oxy");
        assert!(unit.contains("--days 45 --root \"/tmp/Проекты & x\""));
    }
    #[test]
    fn weekdays_match_cli_and_catch_up_missed_runs() {
        assert!(timer(0, 3).contains("OnCalendar=Sun *-*-* 03:00:00"));
        assert!(timer(6, 23).contains("OnCalendar=Sat *-*-* 23:00:00"));
        assert!(timer(1, 0).contains("Persistent=true"));
    }
}
