//! Conservatively pause cleanup while any Cargo/rustc process is visible.
//! Build output can be redirected independently of the process working directory.
use crate::i18n::text;
use anyhow::{bail, Result};
use std::path::Path;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};

fn is_build(name: &std::ffi::OsStr) -> bool {
    matches!(
        name.to_string_lossy().to_ascii_lowercase().as_str(),
        "cargo" | "cargo.exe" | "rustc" | "rustc.exe"
    )
}

pub fn detect_active_builds(_root: &Path) -> Result<Vec<String>> {
    if !sysinfo::IS_SUPPORTED_SYSTEM {
        bail!(text("process_unavailable"));
    }
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    if system.processes().is_empty() {
        bail!(text("process_unavailable"));
    }
    Ok(system
        .processes()
        .iter()
        .filter(|(_, process)| is_build(process.name()))
        .map(|(pid, process)| format!("{pid}: {}", process.name().to_string_lossy()))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_only_build_executables_on_all_platforms() {
        for name in ["cargo", "rustc", "cargo.exe", "RUSTC.EXE"] {
            assert!(is_build(name.as_ref()));
        }
        for name in ["cargo-sweep", "cargo-sweep.exe", "editor", "my-cargo-tool"] {
            assert!(!is_build(name.as_ref()));
        }
    }
}
