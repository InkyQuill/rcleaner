# Authorship and third-party notices

## oxicleaner — original project

- Project: https://github.com/a7garden/oxicleaner
- Author: **a7garden** (`a7garden@icloud.com`).
- Copyright: **Copyright (c) 2026 a7garden**.
- License: **MIT**; the original text is preserved verbatim at
  [licenses/oxicleaner-MIT.txt](licenses/oxicleaner-MIT.txt).
- Fork point in this repository: `fdad842` (original Git history is retained).

**rcleaner** is a derivative fork, maintained by **Pavel Obruchnikov**
(InkyQuill, `me@inkyquill.net`). The fork adds Windows/Linux schedulers, locale-based
English/Russian/Korean UI, embedded cleanup and release packaging. These changes
are also MIT-licensed. Original authorship is not attributed to the fork maintainer.

## cargo-sweep — embedded cleanup engine

- Project: https://github.com/holmgr/cargo-sweep
- Source version: crates.io **cargo-sweep 0.8.0**, `src/fingerprint.rs`.
- Original author: **Viktor Holmgren** (`viktor.holmgren@gmail.com`) and contributors.
- Copyright: **Copyright (c) 2018 Viktor Holmgren**.
- License: **MIT**, preserved at [vendor/cargo-sweep/LICENSE](vendor/cargo-sweep/LICENSE).
- Adaptation: [vendor/cargo-sweep/README.md](vendor/cargo-sweep/README.md).

The embedded derivative implements only the age-based cleanup used by rcleaner;
it is not a claim that the complete upstream CLI is included unchanged.

Release archives include both original license notices, this file and the fork's
LICENSE. Other Rust dependencies remain subject to their respective licenses as
identified by their Cargo package metadata and Cargo.lock.
