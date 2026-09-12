use std::process::Command;

fn command(home: &std::path::Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_rcleaner"));
    command
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("CARGO_HOME", home.join(".cargo"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("RCLEANER_LANG", "en");
    command
}

#[test]
fn help_is_translated_and_unknown_locales_fall_back() {
    let home = tempfile::tempdir().unwrap();
    for (locale, expected) in [
        ("ru_RU.UTF-8", "Очистка Rust"),
        ("ko-KR", "시스템 스케줄"),
        ("en-US", "Recursive Rust"),
        ("fr-FR", "Recursive Rust"),
    ] {
        let output = command(home.path())
            .env("RCLEANER_LANG", locale)
            .arg("--help")
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains(expected));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn system_locale_is_used_without_override() {
    let home = tempfile::tempdir().unwrap();
    let output = command(home.path())
        .env_remove("RCLEANER_LANG")
        .env_remove("LANGUAGE")
        .env("LC_ALL", "ru_RU.UTF-8")
        .arg("--help")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&output.stdout).contains("Очистка Rust"));
}

#[test]
fn malformed_configuration_does_not_silently_use_defaults() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir(home.path().join(".rcleaner")).unwrap();
    std::fs::write(home.path().join(".rcleaner/config.toml"), "invalid = [").unwrap();
    let output = command(home.path())
        .args(["sweep", "--dry-run"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Cannot read configuration"));
}

#[cfg(unix)]
fn script(path: &std::path::Path, contents: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, contents).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(unix)]
#[test]
fn dry_run_uses_embedded_engine_and_config_retention() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join("Проекты & spaces");
    std::fs::create_dir_all(root.join("target")).unwrap();
    let sentinel = root.join("target/keep");
    std::fs::write(&sentinel, "keep me").unwrap();
    let bin = home.path().join(".cargo/bin");
    std::fs::create_dir_all(&bin).unwrap();
    script(
        &bin.join("cargo"),
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$HOME/args\"\n/bin/cat \"$HOME/metadata.json\"\n",
    );
    std::fs::write(
        home.path().join("metadata.json"),
        serde_json::json!({"target_directory": root.join("target")}).to_string(),
    )
    .unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='example'\nversion='0.1.0'\n",
    )
    .unwrap();
    let fingerprint = root.join("target/debug/.fingerprint/example-1111111111111111");
    std::fs::create_dir_all(&fingerprint).unwrap();
    let file = std::fs::File::create(fingerprint.join("lib-example")).unwrap();
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(40 * 86_400);
    file.set_times(
        std::fs::FileTimes::new()
            .set_accessed(old)
            .set_modified(old),
    )
    .unwrap();
    std::fs::create_dir_all(root.join("target/debug/deps")).unwrap();
    std::fs::write(
        root.join("target/debug/deps/libexample-1111111111111111.rlib"),
        "artifact",
    )
    .unwrap();
    std::fs::create_dir(home.path().join(".rcleaner")).unwrap();
    std::fs::write(
        home.path().join(".rcleaner/config.toml"),
        format!(
            "root = {}\ndays = 45\n",
            serde_json::to_string(&root).unwrap()
        ),
    )
    .unwrap();
    let output = command(home.path())
        .args(["sweep", "--dry-run"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let args = std::fs::read_to_string(home.path().join("args")).unwrap();
    assert_eq!(
        args,
        format!(
            "metadata\n--no-deps\n--format-version\n1\n--offline\n--manifest-path\n{}\n",
            root.canonicalize().unwrap().join("Cargo.toml").display()
        )
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("Nothing to clean"));
    let preview = command(home.path())
        .args(["sweep", "--days", "30", "--dry-run"])
        .output()
        .unwrap();
    assert!(preview.status.success());
    assert!(String::from_utf8_lossy(&preview.stdout).contains("8.00 B"));
    assert!(root
        .join("target/debug/deps/libexample-1111111111111111.rlib")
        .exists());
    assert_eq!(std::fs::read_to_string(sentinel).unwrap(), "keep me");
    assert!(!home.path().join(".rcleaner/history.jsonl").exists());
    assert!(!bin.join("cargo-sweep").exists());
}

#[cfg(target_os = "linux")]
#[test]
fn schedule_lifecycle_uses_user_manager_and_absolute_paths() {
    let home = tempfile::tempdir().unwrap();
    let bin = home.path().join(".cargo/bin");
    std::fs::create_dir_all(&bin).unwrap();
    script(&bin.join("cargo"), "#!/bin/sh\nexit 0\n");
    script(&bin.join("systemctl"), "#!/bin/sh\nprintf '%s\\n' \"$@\" >> \"$HOME/systemctl-args\"\ncase \"$2\" in show) echo active;; esac\n");
    let output = command(home.path())
        .env("PATH", &bin)
        .current_dir(home.path())
        .args([
            "enable",
            "--root",
            ".",
            "--weekday",
            "5",
            "--hour",
            "4",
            "--days",
            "45",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let units = home.path().join("config/systemd/user");
    let service = std::fs::read_to_string(units.join("rcleaner.service")).unwrap();
    assert!(service.contains(&format!(
        "--root \"{}\"",
        home.path().canonicalize().unwrap().display()
    )));
    assert!(std::fs::read_to_string(units.join("rcleaner.timer"))
        .unwrap()
        .contains("Fri *-*-* 04:00:00"));
    let output = command(home.path())
        .env("PATH", &bin)
        .arg("status")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("state: enabled"));
    // Re-enable using the installed executable: must not copy it over itself.
    let output = Command::new(home.path().join(".rcleaner/rcleaner"))
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path().join("config"))
        .env("PATH", &bin)
        .arg("enable")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = command(home.path())
        .env("PATH", &bin)
        .arg("disable")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!units.join("rcleaner.timer").exists());
    assert!(!units.join("rcleaner.service").exists());
    let calls = std::fs::read_to_string(home.path().join("systemctl-args")).unwrap();
    assert!(calls.contains("--user\ndisable\n--now\nrcleaner.timer"));
}

#[cfg(unix)]
#[test]
fn discovery_skips_permission_denied_children_but_not_root() {
    use std::os::unix::fs::PermissionsExt;
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join("projects");
    let denied = root.join("data-pg");
    let project = root.join("healthy");
    std::fs::create_dir_all(&denied).unwrap();
    std::fs::create_dir_all(project.join("target")).unwrap();
    std::fs::write(project.join("Cargo.toml"), "[package]\nname='healthy'\n").unwrap();
    let bin = home.path().join(".cargo/bin");
    std::fs::create_dir_all(&bin).unwrap();
    script(
        &bin.join("cargo"),
        "#!/bin/sh\n/bin/cat \"$HOME/metadata.json\"\n",
    );
    std::fs::write(
        home.path().join("metadata.json"),
        serde_json::json!({"target_directory": project.join("target")}).to_string(),
    )
    .unwrap();
    std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o000)).unwrap();
    if std::fs::read_dir(&denied).is_ok() {
        std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o755)).unwrap();
        eprintln!("Permission test requires a user without permission bypass (not root)");
        return;
    }
    let child = command(home.path())
        .args(["sweep", "--dry-run", "--root"])
        .arg(&root)
        .output()
        .unwrap();
    let root_result = command(home.path())
        .args(["sweep", "--dry-run", "--root"])
        .arg(&denied)
        .output()
        .unwrap();
    std::fs::set_permissions(&denied, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        child.status.success(),
        "{}",
        String::from_utf8_lossy(&child.stderr)
    );
    assert!(String::from_utf8_lossy(&child.stderr).contains("Skipping inaccessible path"));
    assert!(String::from_utf8_lossy(&child.stderr).contains("data-pg"));
    assert!(String::from_utf8_lossy(&child.stdout).contains("healthy"));
    assert!(
        !root_result.status.success(),
        "An unreadable root must not report success"
    );
}
