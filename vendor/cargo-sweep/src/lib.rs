//! Age-based cargo-sweep engine, adapted from cargo-sweep 0.8.0 (MIT).
//! See ../README.md for provenance and changes from upstream.
use anyhow::Result;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
use walkdir::WalkDir;

fn hash_from_path_name(filename: &str) -> Option<&str> {
    let name = filename.split('.').next()?;
    let (prefix, hash) = name.rsplit_once('-')?;
    if !prefix.is_empty() && hash.len() == 16 && hash.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Some(hash)
    } else {
        None
    }
}

fn stale_fingerprints(dir: &Path, retention: Duration, now: SystemTime) -> Result<HashSet<String>> {
    let mut stale = HashSet::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(hash) = hash_from_path_name(name) else {
            continue;
        };
        let mut has_files = false;
        let mut old = true;
        for fingerprint in fs::read_dir(entry.path())? {
            let fingerprint = fingerprint?;
            if !fingerprint.file_type()?.is_file() {
                old = false;
                break;
            }
            let metadata = fingerprint.metadata()?;
            has_files = true;
            // atime can be disabled or coarse; a recent mtime must always keep it.
            for time in [metadata.accessed()?, metadata.modified()?] {
                if now.duration_since(time).unwrap_or_default() < retention {
                    old = false;
                }
            }
        }
        if has_files && old {
            stale.insert(hash.to_owned());
        }
    }
    Ok(stale)
}

fn size(path: &Path) -> Result<u64> {
    let mut bytes = 0;
    for entry in WalkDir::new(path)
        .follow_links(false)
        .follow_root_links(false)
    {
        let entry = entry?;
        if entry.file_type().is_file() {
            bytes += entry.metadata()?.len();
        }
    }
    Ok(bytes)
}

fn remove_stale(dir: &Path, stale: &HashSet<String>, dry_run: bool) -> Result<u64> {
    let metadata = match fs::symlink_metadata(dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    if !metadata.is_dir() {
        return Ok(0);
    }
    let mut bytes = 0;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !hash_from_path_name(name).is_some_and(|hash| stale.contains(hash)) {
            continue;
        }
        let kind = entry.file_type()?;
        if kind.is_file() {
            let len = entry.metadata()?.len();
            if !dry_run {
                fs::remove_file(entry.path())?;
            }
            bytes += len;
        } else if kind.is_dir() {
            let len = size(&entry.path())?;
            if !dry_run {
                fs::remove_dir_all(entry.path())?;
            }
            bytes += len;
        }
    }
    Ok(bytes)
}

/// Sweep fingerprint-associated artifacts older than `retention` in a target directory.
/// Returns logical bytes removed (or removable for a dry-run), not filesystem blocks.
pub fn remove_older_than(target: &Path, retention: Duration, dry_run: bool) -> Result<u64> {
    let mut fingerprints: Vec<PathBuf> = Vec::new();
    let mut walk = WalkDir::new(target)
        .follow_links(false)
        .follow_root_links(false)
        .into_iter();
    while let Some(entry) = walk.next() {
        let entry = entry?;
        if entry.file_type().is_dir() && entry.file_name() == ".fingerprint" {
            fingerprints.push(entry.into_path());
            walk.skip_current_dir();
        }
    }
    let now = SystemTime::now();
    let mut bytes = 0;
    for fingerprint in fingerprints {
        let stale = stale_fingerprints(&fingerprint, retention, now)?;
        let profile = fingerprint.parent().expect("fingerprint has a parent");
        for directory in [
            profile.join("build"),
            profile.join("deps"),
            profile.join("native"),
            profile.to_path_buf(),
            fingerprint,
        ] {
            bytes += remove_stale(&directory, &stale, dry_run)?;
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    const OLD: &str = "1111111111111111";
    const NEW: &str = "2222222222222222";
    fn artifact(root: &Path, hash: &str, days: u64) -> PathBuf {
        let fingerprint = root.join(format!("debug/.fingerprint/example-{hash}"));
        fs::create_dir_all(&fingerprint).unwrap();
        let file = fs::File::create(fingerprint.join("lib-example")).unwrap();
        let timestamp = SystemTime::now() - Duration::from_secs(days * 86_400);
        file.set_times(
            fs::FileTimes::new()
                .set_accessed(timestamp)
                .set_modified(timestamp),
        )
        .unwrap();
        fs::create_dir_all(root.join("debug/deps")).unwrap();
        let artifact = root.join(format!("debug/deps/libexample-{hash}.rlib"));
        fs::write(&artifact, "1234567890").unwrap();
        artifact
    }
    #[test]
    fn dry_run_and_live_keep_recent_artifacts() {
        let root = tempfile::tempdir().unwrap();
        let old = artifact(root.path(), OLD, 60);
        let new = artifact(root.path(), NEW, 1);
        let retention = Duration::from_secs(30 * 86_400);
        assert_eq!(remove_older_than(root.path(), retention, true).unwrap(), 10);
        assert!(old.exists());
        assert_eq!(
            remove_older_than(root.path(), retention, false).unwrap(),
            10
        );
        assert!(!old.exists());
        assert!(new.exists());
    }
    #[test]
    fn unknown_hashes_and_empty_fingerprints_are_preserved() {
        let root = tempfile::tempdir().unwrap();
        let artifact = artifact(root.path(), OLD, 60);
        fs::remove_file(
            root.path()
                .join(format!("debug/.fingerprint/example-{OLD}/lib-example")),
        )
        .unwrap();
        assert_eq!(
            remove_older_than(root.path(), Duration::from_secs(86_400), false).unwrap(),
            0
        );
        assert!(artifact.exists());
    }
    #[cfg(unix)]
    #[test]
    fn symlinks_never_clean_external_artifacts() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let artifact = artifact(outside.path(), OLD, 60);
        std::os::unix::fs::symlink(outside.path().join("debug"), root.path().join("debug"))
            .unwrap();
        assert_eq!(
            remove_older_than(root.path(), Duration::from_secs(86_400), false).unwrap(),
            0
        );
        assert!(artifact.exists());
    }
}
