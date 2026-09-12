//! launchd 스케줄 자동 관리. plist 를 생성하고 bootstrap/bootout 한다.

use super::checked;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// launchd 라벨 (plist Label 및 launchctl 식별자).
pub const LABEL: &str = "local.rcleaner";

/// plist 파일 경로: `~/Library/LaunchAgents/local.rcleaner.plist`
fn plist_path(home: &Path) -> PathBuf {
    home.join("Library/LaunchAgents/local.rcleaner.plist")
}

/// launchd 로그 디렉토리: `~/Library/Logs/rcleaner/`
fn log_dir(home: &Path) -> PathBuf {
    home.join("Library/Logs/rcleaner")
}

/// 스케줄을 활성화(설치/갱신) 한다.
///
/// - `weekday`: 0(일) ~ 6(토)
/// - `hour`: 0 ~ 23
/// - `days`: 보존 일수 (sweep 에 전달)
/// - `root`: 재귀 스캔 루트
/// - `binary`: rcleaner 실행파일 절대경로 (plist 에 박힘)
pub fn enable(weekday: u32, hour: u32, days: u32, root: &str, binary: &str) -> Result<()> {
    let home = super::required_home(crate::config::home_dir())?;
    disable_at(&home)?;

    fs::create_dir_all(log_dir(&home))?;

    let plist = render_plist(&home, weekday, hour, days, root, binary);
    let path = plist_path(&home);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, &plist)?;

    let uid = uid()?;
    let domain = format!("gui/{uid}");
    checked(Command::new("launchctl").args(["bootstrap", &domain, &path.to_string_lossy()]))?;
    Ok(())
}

/// 스케줄을 비활성화(제거)한다. plist 파일도 삭제. (로드되어 있지 않아도 조용히 통과.)
pub fn disable() -> Result<()> {
    let home = super::required_home(crate::config::home_dir())?;
    disable_at(&home)
}

fn disable_at(home: &Path) -> Result<()> {
    let uid = uid()?;
    let target = format!("gui/{uid}/{LABEL}");
    if is_loaded()? {
        checked(Command::new("launchctl").args(["bootout", &target]))?;
    }
    let p = plist_path(home);
    if p.exists() {
        fs::remove_file(&p)?;
    }
    Ok(())
}

/// 스케줄이 현재 launchd 에 로드되어 있는지.
pub fn is_loaded() -> Result<bool> {
    let uid = uid()?;
    let out = Command::new("launchctl")
        .args(["print", &format!("gui/{uid}/{LABEL}")])
        .output()?;
    if out.status.success() {
        return Ok(true);
    }
    // launchctl returns 113 when this service is absent; other failures matter.
    if out.status.code() == Some(113) {
        return Ok(false);
    }
    anyhow::bail!(crate::i18n::message(
        "command_failed",
        &[&"launchctl print", &String::from_utf8_lossy(&out.stderr)]
    ))
}

fn render_plist(
    home: &Path,
    weekday: u32,
    hour: u32,
    days: u32,
    root: &str,
    binary: &str,
) -> String {
    let path_env = format!(
        "{}/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin",
        home.display()
    );
    let log_out = log_dir(home)
        .join("launchd.out.log")
        .to_string_lossy()
        .into_owned();
    let log_err = log_dir(home)
        .join("launchd.err.log")
        .to_string_lossy()
        .into_owned();

    // XML 엔티티 이스케이프 — 경로/바이너리에 & < > " 가 있으면 plist 가 깨진다.
    fn esc(s: &str) -> String {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    }
    let binary_esc = esc(binary);
    let root_esc = esc(root);
    let home_esc = esc(&home.to_string_lossy());
    let path_env_esc = esc(&path_env);

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>

    <key>ProgramArguments</key>
    <array>
        <string>{binary}</string>
        <string>sweep</string>
        <string>--days</string>
        <string>{days}</string>
        <string>--root</string>
        <string>{root}</string>
    </array>

    <key>StartCalendarInterval</key>
    <dict>
        <key>Weekday</key>
        <integer>{weekday}</integer>
        <key>Hour</key>
        <integer>{hour}</integer>
        <key>Minute</key>
        <integer>0</integer>
    </dict>

    <key>ProcessType</key>
    <string>Background</string>
    <key>LowPriorityIO</key>
    <true/>

    <key>StandardOutPath</key>
    <string>{log_out}</string>
    <key>StandardErrorPath</key>
    <string>{log_err}</string>

    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>{path_env}</string>
        <key>HOME</key>
        <string>{home}</string>
    </dict>
</dict>
</plist>
"#,
        label = LABEL,
        binary = binary_esc,
        days = days,
        root = root_esc,
        weekday = weekday,
        hour = hour,
        log_out = esc(&log_out),
        log_err = esc(&log_err),
        path_env = path_env_esc,
        home = home_esc,
    )
}

fn uid() -> Result<String> {
    let out = checked(Command::new("id").arg("-u"))?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// render_plist: 반드시 유효한 XML plist 여야 한다.
    /// XML 선언, DOCTYPE, plist/schema 가 포함되어야 하고 인자가 위치에 맞게 삽입되어야 함.
    #[test]
    fn test_render_plist_basic() {
        let plist = render_plist(
            Path::new("/Users/test & home"),
            0,
            3,
            30,
            "/Volumes/MERCURY/PROJECTS",
            "/usr/bin/rcleaner",
        );
        assert!(plist.starts_with(r#"<?xml"#));
        assert!(plist.contains("<integer>0</integer>"), "weekday 0");
        assert!(plist.contains("<integer>3</integer>"), "hour 3");
        // days 는 ProgramArguments 에서 <string> 으로 쓰임
        assert!(plist.contains("<string>30</string>"), "days 30");
        assert!(plist.contains("ProgramArguments"));
        assert!(plist.contains("StartCalendarInterval"));
        assert!(plist.contains("EnvironmentVariables"));
    }

    /// XML escape: 특수 문자가 올바르게 변환되는지.
    #[test]
    fn test_render_plist_xml_escape() {
        let binary = "/Users/x&y/bin/<rcleaner>\".exe";
        let root = "/path/with/\"quotes\"&ampersands";
        let plist = render_plist(Path::new("/Users/test & home"), 0, 3, 30, root, binary);
        // 원래 문자는 XML 에 없어야 함
        assert!(!plist.contains("&y"), "& 는 &amp; 로 이스케이프되어야 함");
        assert!(
            !plist.contains("<rcleaner"),
            "< 는 &lt; 로 이스케이프되어야 함"
        );
        assert!(
            !plist.contains("\"quotes"),
            "\" 는 &quot; 로 이스케이프되어야 함"
        );
        // 이스케이프된 버전이 있어야 함
        assert!(plist.contains("x&amp;y"));
        assert!(plist.contains("&lt;rcleaner"));
        assert!(plist.contains("&quot;quotes"));
        assert!(
            plist.contains("&amp;ampersands"),
            "&amp; 의 & 도 이스케이프 필요"
        );
    }
}
