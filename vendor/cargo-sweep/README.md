# Embedded cargo-sweep age-based engine

Derived from `src/fingerprint.rs` in the crates.io cargo-sweep **0.8.0** release
(https://github.com/holmgr/cargo-sweep), under the MIT license in LICENSE.

This local library exposes only the age-based cleanup used by rcleaner.
Toolchain, size-limit and CLI code are omitted. Local changes:

- Return byte counts and propagate filesystem failures instead of logging and
  reporting failed deletions as reclaimed space.
- Do not follow symlinks in traversal, fingerprints or deletion candidates.
- Keep artifacts with missing fingerprints, empty fingerprints, or recent
  modification/access timestamps. Only positively identified stale hashes delete.
- Collect fingerprint directories before mutating the tree; ignore absent
  optional profile directories.
- Tests exercise dry-run, old/recent artifacts and symlink boundaries.

Review these changes explicitly when updating the upstream base. Cargo uses
internal fingerprint formats and timestamps: this is a heuristic, not a guarantee
that a rebuild will never be needed. Incremental directories are not removed.
