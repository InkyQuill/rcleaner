//! Embedded cleanup with native process guards and structured reports.
use crate::{
    config, disk,
    i18n::{message, text},
    safety,
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

/// 단일 프로젝트의 정리 결과.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweptProject {
    /// target 디렉토리 경로.
    pub path: String,
    /// 확보한 크기 (예: "516.33 MiB"). 정리 대상 없으면 None.
    pub freed: Option<String>,
}

/// 한 번의 sweep 실행 리포트. 히스토리(JSONL)에 한 줄로 저장된다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SweepReport {
    /// 실행 시각 (RFC3339).
    pub timestamp: String,
    /// "dry-run" | "live"
    pub mode: String,
    /// 보존 일수.
    pub days: u32,
    /// 스캔 루트.
    pub root: String,
    /// 프로젝트별 결과.
    pub projects: Vec<SweptProject>,
    /// 총 확보량 (예: "42.47 GiB").
    pub total_freed: Option<String>,
    /// 디스크 사용 현황 (실행 전).
    pub disk_before: String,
    /// 디스크 사용 현황 (실행 후).
    pub disk_after: String,
    /// 빌드 중 등으로 스킵했는지.
    pub skipped: bool,
    /// 스킵 사유.
    pub skip_reason: Option<String>,
}

pub fn run(root: &Path, days: u32, dry_run: bool, force: bool) -> Result<SweepReport> {
    let root = config::validate(root, days)?;
    std::fs::create_dir_all(config::config_dir())?;
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(config::config_dir().join("sweep.lock"))?;
    fs2::FileExt::try_lock_exclusive(&lock).context(text("locked"))?;
    let disk_before = disk::stat(&root);
    let mut report = SweepReport {
        timestamp: chrono::Local::now().to_rfc3339(),
        mode: if dry_run { "dry-run" } else { "live" }.into(),
        days,
        root: root.to_string_lossy().into_owned(),
        projects: vec![],
        total_freed: None,
        disk_after: disk_before.clone(),
        disk_before,
        skipped: false,
        skip_reason: None,
    };
    if !force && !dry_run {
        let builds = safety::detect_active_builds(&root)?;
        if !builds.is_empty() {
            report.skipped = true;
            report.skip_reason = Some(message("builds", &[&builds.len()]));
            return Ok(report);
        }
    }
    let targets = find_targets(&root)?;
    if !dry_run {
        // Never clean an ancestor of the running binary, even with a custom target-dir.
        let executable = std::env::current_exe()?.canonicalize()?;
        for target in &targets {
            if executable.starts_with(target) {
                bail!(message("self_target", &[&target.display()]));
            }
        }
    }
    // Metadata discovery may take time; check again immediately before each target.
    let mut total = 0;
    for target in targets {
        if !force && !dry_run {
            let builds = safety::detect_active_builds(&root)?;
            if !builds.is_empty() {
                report.skipped = true;
                report.skip_reason = Some(message("builds", &[&builds.len()]));
                break;
            }
        }
        let bytes = rcleaner_sweep::remove_older_than(
            &target,
            Duration::from_secs(u64::from(days) * 86_400),
            dry_run,
        )
        .with_context(|| message("cleanup_failed", &[&target.display()]))?;
        total += bytes;
        report.projects.push(SweptProject {
            path: target.to_string_lossy().into_owned(),
            freed: (bytes > 0).then(|| format_bytes(bytes)),
        });
    }
    report.total_freed = Some(format_bytes(total));
    report.disk_after = disk::stat(&root);
    Ok(report)
}

#[derive(Deserialize)]
struct Metadata {
    target_directory: PathBuf,
}

fn find_targets(root: &Path) -> Result<BTreeSet<PathBuf>> {
    let cargo = find_cargo()?;
    let mut targets = BTreeSet::new();
    let mut walk = walkdir::WalkDir::new(root).follow_links(false).into_iter();
    while let Some(entry) = walk.next() {
        let entry = entry?;
        if entry.file_type().is_dir() && entry.depth() > 0 {
            let name = entry.file_name().to_string_lossy();
            if name.starts_with('.') || name == "target" || targets.contains(entry.path()) {
                walk.skip_current_dir();
            }
            continue;
        }
        if !entry.file_type().is_file() || entry.file_name() != "Cargo.toml" {
            continue;
        }
        let output = Command::new(&cargo)
            .args([
                "metadata",
                "--no-deps",
                "--format-version",
                "1",
                "--offline",
                "--manifest-path",
            ])
            .arg(entry.path())
            .current_dir(entry.path().parent().unwrap_or(root))
            .env("PATH", enhanced_path()?)
            .output()
            .context(text("metadata_failed"))?;
        if !output.status.success() {
            bail!(message(
                "command_failed",
                &[
                    &format!("cargo metadata ({})", entry.path().display()),
                    &String::from_utf8_lossy(&output.stderr)
                ]
            ));
        }
        let metadata: Metadata =
            serde_json::from_slice(&output.stdout).context(text("metadata_failed"))?;
        if metadata.target_directory.is_dir() {
            targets.insert(metadata.target_directory.canonicalize()?);
        }
    }
    Ok(targets)
}

pub fn find_cargo() -> Result<PathBuf> {
    if let Some(path) = config::load()?.and_then(|cfg| cfg.cargo) {
        if path.is_file() {
            return Ok(path);
        }
    }
    // Do not canonicalize rustup proxies: argv[0] must remain `cargo`.
    which::which_in(
        format!("cargo{}", std::env::consts::EXE_SUFFIX),
        Some(enhanced_path()?),
        std::env::current_dir()?,
    )
    .map_err(|_| anyhow::anyhow!(text("missing_cargo")))
}

fn enhanced_path() -> Result<std::ffi::OsString> {
    let mut paths = Vec::new();
    if let Some(cargo_home) = std::env::var_os("CARGO_HOME") {
        paths.push(PathBuf::from(cargo_home).join("bin"));
    }
    if let Some(home) = crate::config::home_dir() {
        paths.push(home.join(".cargo/bin"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    #[cfg(unix)]
    paths.extend(["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"].map(PathBuf::from));
    Ok(std::env::join_paths(paths)?)
}

fn format_bytes(bytes: u64) -> String {
    let mut value = bytes as f64;
    let mut unit = "B";
    for next in ["KiB", "MiB", "GiB", "TiB"] {
        if value < 1024.0 {
            break;
        }
        value /= 1024.0;
        unit = next;
    }
    format!("{value:.2} {unit}")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn byte_counts_are_not_parsed_from_console_output() {
        assert_eq!(format_bytes(0), "0.00 B");
        assert_eq!(format_bytes(1_073_741_824), "1.00 GiB");
    }
}
