# rcleaner

A cross-platform CLI for cleaning old Rust build artifacts across multiple Cargo
projects, with native weekly scheduling and English, Russian and Korean interfaces.

**rcleaner is a fork of [oxicleaner](https://github.com/a7garden/oxicleaner),
originally written by [a7garden](https://github.com/a7garden)
(`a7garden@icloud.com`) under the MIT license.** This fork is maintained by
Pavel Obruchnikov (InkyQuill, `me@inkyquill.net`). It adds Linux and Windows
support alongside macOS, system-language selection, an embedded cleanup engine
and downloadable release packaging.

The [original oxicleaner MIT license](licenses/oxicleaner-MIT.txt) is preserved
verbatim. See [LICENSE](LICENSE) for this fork and [THIRD_PARTY.md](THIRD_PARTY.md)
for the embedded cargo-sweep attribution.

## Install

Download the archive for your platform from
[Releases](https://github.com/InkyQuill/rcleaner/releases), extract it, and place
`rcleaner` (`rcleaner.exe` on Windows) in a directory on your `PATH`.
Each archive has a `.sha256` checksum file.

| Platform | Release target | Scheduler |
| --- | --- | --- |
| Linux x64 | `x86_64-unknown-linux-gnu` | systemd user timer |
| Linux ARM64 | `aarch64-unknown-linux-gnu` | systemd user timer |
| macOS Intel | `x86_64-apple-darwin` | launchd LaunchAgent |
| macOS Apple Silicon | `aarch64-apple-darwin` | launchd LaunchAgent |
| Windows x64 | `x86_64-pc-windows-msvc` | Task Scheduler |

Linux release binaries are built on Ubuntu 22.04 and require glibc 2.35 or newer.
On older distributions or musl-based systems, build from source.

**No separate `cargo-sweep` installation is required.** Its age-based cleanup
engine is embedded in this binary. Cargo from your Rust toolchain is still needed:
`cargo metadata --offline --no-deps` locates each project's actual target directory,
including workspace and custom `target-dir` settings. The tool searches
`CARGO_HOME/bin`, `~/.cargo/bin` and `PATH`. Metadata failures abort cleanup instead
of silently skipping a project. A project must be resolvable offline by Cargo.

To build from source:

```sh
git clone https://github.com/InkyQuill/rcleaner.git
cd rcleaner
cargo install --path . --locked
```

Install outside `target/`: live cleanup refuses to clean a directory containing
the running executable.

## Usage

```sh
# Interactive setup: root, retention, weekly schedule, optional first cleanup
rcleaner setup

# Preview first, keeping artifacts used in the last 30 days
rcleaner sweep --root /path/to/projects --days 30 --dry-run

# Clean now; omitted root/days use config.toml, then current directory/30
rcleaner sweep
rcleaner sweep --days 60

# Fridays at 04:00 local time, retain 45 days
rcleaner enable --root /path/to/projects --weekday 5 --hour 4 --days 45

rcleaner status
rcleaner history --limit 20
rcleaner disable
```

On Windows, use paths such as `--root "C:\Users\Pavel\Projects"`. Paths containing
spaces or Unicode are supported. Schedule paths are stored as absolute paths.

No subcommand runs a live sweep with the saved settings. `--force` bypasses the
build-process guard, but does not bypass the lock preventing overlapping
rcleaner runs. `--days` must be positive; weekday is 0 (Sunday) through 6
(Saturday), and hour is 0 through 23.

## Languages

The interface automatically selects **English (`en`), Russian (`ru`) or Korean
(`ko`)** using the system locale. Regional variants such as `ru_RU.UTF-8`, `ru-RU`
and `ko-KR` map to the corresponding language. Other locales fall back to English.

On Linux, locale discovery uses `LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, then `LANG`.
On macOS and Windows it uses native system language APIs. Scheduled runs use the
scheduler's environment/system language, which can differ from an interactive
terminal's environment.

For an explicit override:

```sh
RCLEANER_LANG=ru rcleaner --help
```

```powershell
$env:RCLEANER_LANG = 'ru'
rcleaner --help
```

Descriptions, help headings, setup prompts, application errors and reports are
translated. Command names and JSON field names are stable. Diagnostics originating
from Cargo, the OS, clap's argument parser or the terminal library retain their
own language. Existing history records retain the text recorded at execution time.

Translation catalogs are in `locales/en.json`, `locales/ru.json` and
`locales/ko.json`, embedded at compile time. Tests verify matching keys and
numbered placeholders across catalogs.

## What gets cleaned

The scanner finds Cargo manifests below the root, skipping hidden directories,
symlink traversal and conventional `target/` directories. Cargo metadata resolves
actual target directories and shared workspace targets are deduplicated.
Permission-denied errors below the scan root produce a warning and skip that
path, allowing accessible projects to be found. An unreadable scan root and
other discovery or artifact-deletion errors still abort the run. A custom
Cargo target directory may be **outside** the scan root; inspect `--dry-run` output.

The embedded engine derives from cargo-sweep 0.8.0 and uses Cargo fingerprint
hashes and file access/modification times to identify old artifacts. It removes
matching artifacts in profile `deps`, `build`, legacy `native`, fingerprint and
profile directories. It preserves recent artifacts, unknown hashes and empty
fingerprints. It does not remove incremental caches or arbitrary untracked files.
See the [engine provenance and changes](vendor/cargo-sweep/README.md).

Filesystem timestamps are a heuristic: disabled/coarse access-time updates can
make age estimates inaccurate, and deleted artifacts may need rebuilding. The
reported size is logical file size, not guaranteed newly available disk blocks.

Before discovery and again before each target in a live run, the process guard postpones cleanup if it sees **any** `cargo`
or `rustc` process, including Windows `.exe` names. This is conservative because
build output can be redirected independently of process working directories.
Process inspection failure aborts cleanup. This is a snapshot check, not a lock
against a build starting later; schedule cleanup outside your build hours.
A separate OS-backed lock prevents simultaneous rcleaner runs.

Live runs and build-related skips are recorded in history. Dry-runs do not append
history. Filesystem or metadata failures return a nonzero exit status; a failure
after deletion has begun can leave a partially completed cleanup.

## Scheduling

`enable` copies the executable to `~/.rcleaner/`, records the Cargo executable
path and installs a per-user weekly schedule. It does not need administrator
privileges under normal user scheduler policy.

- **Linux:** requires a working `systemctl --user` session. Timers use
  `Persistent=true` to catch up missed runs when the user manager starts, including
  immediately after enabling. User services normally follow login sessions;
  running while logged out requires separately configured user lingering. Linux
  without systemd supports manual cleanup, but not `enable`.
- **macOS:** installs a LaunchAgent in the current user's GUI launchd domain.
  The user must have a GUI login session. The LaunchAgent is named `local.rcleaner`.
- **Windows:** registers a weekly task for the current user's SID with an
  interactive token and limited privileges. It runs while that user is logged in,
  with catch-up enabled. Default Task Scheduler battery/sleep restrictions apply.
  No password is stored. The Windows PowerShell `ScheduledTasks` module is required.

`disable` removes future scheduled triggers. It does not interrupt a cleanup
already in progress. To change retention for scheduled execution, run `enable`
again with the new `--days` value.

| Location | Purpose |
| --- | --- |
| `~/.rcleaner/config.toml` | Root, retention and Cargo path |
| `~/.rcleaner/rcleaner[.exe]` | Installed scheduled executable |
| `~/.rcleaner/history.jsonl` | Run history, compatible with existing records |
| `~/.rcleaner/sweep.lock` | Lock file; not a running-state indicator |
| `$XDG_CONFIG_HOME/systemd/user/rcleaner.{service,timer}` | Linux units; config directory defaults to `~/.config` |
| `~/Library/LaunchAgents/local.rcleaner.plist` | macOS LaunchAgent |
| `~/Library/Logs/rcleaner/` | macOS scheduler stdout/stderr |
| Task Scheduler → `Rcleaner-<user SID>` | Windows task |

On Windows, `~` denotes the user's profile directory. Linux scheduler output is
available with `journalctl --user -u rcleaner.service`. Windows Task Scheduler
shows task status/exit codes; cleanup results are in `rcleaner history`.

The original `root`/`days` configuration format remains supported. To migrate
from oxicleaner, first run `oxicleaner disable` to stop the old schedule, then
copy `config.toml` and optionally `history.jsonl` from `~/.oxicleaner/` into
`~/.rcleaner/`. Run `rcleaner setup` or `rcleaner enable` to install the new
schedule. rcleaner does not modify or remove the old installation automatically.
Re-run `rcleaner enable` after upgrades to update the scheduled executable.

## Development and releases

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo doc --workspace --no-deps --all-features --locked
```

CI runs on Linux, macOS and Windows. Tests use temporary data and scheduler doubles;
unit generation tests do not substitute for a real scheduler lifecycle test on
each OS.

The `Release binaries` workflow builds all five targets above and packages each
executable with README, licenses and a SHA-256 checksum. A manual workflow run
produces downloadable Actions artifacts. Pushing a `v<version>` tag matching
`Cargo.toml` creates a **draft GitHub Release** after all builds and tests succeed;
review and publish that draft. The workflow does not create tags or increment
versions. Release executables are not code-signed or notarized.

The packaging script can also be run locally (Python 3.11+):

```sh
cargo build --release --locked --target x86_64-unknown-linux-gnu
python scripts/package.py x86_64-unknown-linux-gnu
```

## License

MIT; see [LICENSE](LICENSE). The embedded cargo-sweep derivative retains its
[original MIT license](vendor/cargo-sweep/LICENSE), also included in release archives.
