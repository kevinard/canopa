# Vendored layer-shell source provenance

Research date: 2026-09-16. Scope: Notify's `vendor/iced_layershell` and `vendor/layershellev` at 0.18.1, and the corresponding 0.18.0 trees in the staged Niribar checkout at `.scratch/freedesktop-completion-review/niribar-work`. Implementation and vendor files were inspected read-only. This note records verified artifacts, source correspondence, and license-text sources.

## Verified releases and revisions

Both crates belong to [waycrate/exwlshelleventloop](https://github.com/waycrate/exwlshelleventloop). Their normalized package manifests identify that repository, declare `license = "MIT"`, and list Decodertalkers and Aakash Sen Sharma as package authors. The upstream workspace manifests at both relevant tags carry the same repository and license declaration. [0.18.1 workspace manifest](https://github.com/waycrate/exwlshelleventloop/blob/4c172e9db9191e37e7dc6918740a6386f22acfdd/Cargo.toml), [0.18.0 workspace manifest](https://github.com/waycrate/exwlshelleventloop/blob/e612516167cff2c5639383ab2ccd27c0a4a70c99/Cargo.toml)

| Consumer baseline | Upstream tag | Verified tag commit | Tag archive |
| --- | --- | --- | --- |
| Notify, both crates 0.18.1 | `v0.18.1` | `4c172e9db9191e37e7dc6918740a6386f22acfdd` | [GitHub archive](https://codeload.github.com/waycrate/exwlshelleventloop/tar.gz/refs/tags/v0.18.1) |
| Staged Niribar, both crates 0.18.0 | `v0.18.0` | `e612516167cff2c5639383ab2ccd27c0a4a70c99` | [GitHub archive](https://codeload.github.com/waycrate/exwlshelleventloop/tar.gz/refs/tags/v0.18.0) |

Verification: the upstream [tag listing](https://github.com/waycrate/exwlshelleventloop/tags) names both releases. Downloaded tag archives contain the full commit IDs in their Git archive PAX `comment` headers. More importantly, the published crate content was compared with the corresponding subtree of each tag: all **23 non-generated iced_layershell files** and all **9 non-generated layershellev files** match byte-for-byte for each version. The comparison maps published `Cargo.toml.orig` to the upstream subtree's `Cargo.toml` and excludes generated normalized `Cargo.toml` and package `Cargo.lock`.

This establishes that the published source content matches those tag revisions. It does not claim that the crate publisher's working checkout was necessarily at exactly that commit: none of the four published crate archives contains `.cargo_vcs_info.json`. Cargo documents that VCS metadata is included when available and is only a best-effort snapshot, not a provenance guarantee. [Cargo packaging behavior](https://doc.rust-lang.org/cargo/commands/cargo-package.html)

## Published archive checksums

Each SHA-256 below was computed over the complete `.crate` archive and matched the corresponding checksum in the local cached crates.io sparse-index entry. The 0.18.1 archives were already in the Cargo cache. The 0.18.0 archives were downloaded from the official static.crates.io URLs below into `/tmp` for comparison.

| Package artifact | SHA-256 |
| --- | --- |
| [iced_layershell 0.18.1](https://static.crates.io/crates/iced_layershell/iced_layershell-0.18.1.crate) | `79bb5126427f0d16c37f0336a4c2c800b428c3438e244e75322de8e671b51a5d` |
| [layershellev 0.18.1](https://static.crates.io/crates/layershellev/layershellev-0.18.1.crate) | `88f48eaa0c702d491d6411620eca91eae3beecc018e79402708f97e1cc948ad8` |
| [iced_layershell 0.18.0](https://static.crates.io/crates/iced_layershell/iced_layershell-0.18.0.crate) | `e9af23def8028c786e97b72dc3b596968ad23bca4faa865ec96a134ffe3d71ca` |
| [layershellev 0.18.0](https://static.crates.io/crates/layershellev/layershellev-0.18.0.crate) | `aa93feb9ad2defd5a7420460a8491649cb81709d84cb3759135069624f8ee9a1` |

The cached index records publish times of 2026-05-09 02:40:38/36 UTC for iced_layershell/layershellev 0.18.1 and 2026-05-01 06:02:03/00 UTC for 0.18.0. Those timestamps are cached registry evidence, not dates inferred from this repository's commits.

## Exact upstream license and attribution

Both downloaded tag archives contain one root `LICENSE` file and no other files named as licenses, notices, copying instructions, or author lists. The two root license files are identical, including this attribution:

> Copyright (c) 2023 DecodeTalkers

The full file is the MIT license. Its SHA-256 is `f42393dbd25860cd6e95c3da8aa4d7fde6fe60eedaccf44a72155f90a1556c6c`. Preserve the spelling and capitalization in the notice; it differs from the package author's spelling in Cargo metadata. [Exact 0.18.1 license](https://github.com/waycrate/exwlshelleventloop/blob/4c172e9db9191e37e7dc6918740a6386f22acfdd/LICENSE), [exact 0.18.0 license](https://github.com/waycrate/exwlshelleventloop/blob/e612516167cff2c5639383ab2ccd27c0a4a70c99/LICENSE)

None of the four published `.crate` archives includes that root license text, and none of the four local vendor directories contained a license text when inspected. Their manifests' MIT declaration and the matching upstream source establish where to retrieve it. The MIT text requires preserving its copyright and permission notices with copies or substantial portions. The practical release fix is to copy the exact upstream `LICENSE` into **each vendored crate directory**, preserving its original attribution, and identify the crate/version/commit/checksum in a short provenance file. [MIT license text and condition](https://opensource.org/license/mit)

There was no separate upstream `NOTICE` to copy from these releases. The README describes taking inspiration/reference from winit; that statement alone does not establish a separate exact copied source revision or license attribution. This bounded investigation found no additional source-header copyright/license notices inside these two crate subtrees. Do not invent an Iced or winit revision or replace the verified upstream attribution with a guess. Dependency attributions for a complete compiled release are broader than these two vendor baselines and should be generated from the final dependency set.

## Local modifications, as inspected

These comparisons describe the working trees at research time. Later implementation changes can extend the list; the crates remain locally modified copies of the verified baselines.

| Local tree | Matches published archive | Changed archive files | Added files | Omitted archive files |
| --- | --- | --- | --- | --- |
| Notify `vendor/iced_layershell` | 22 files | `src/actions.rs`, `src/multi_window.rs` | `src/multi_window/tests.rs` | `Cargo.lock` |
| Notify `vendor/layershellev` | 7 files | `src/events.rs`, `src/lib.rs`, `src/seat.rs` | none | `Cargo.lock` |
| Staged Niribar `vendor/iced_layershell` | 16 files | `Cargo.toml`, `src/actions.rs`, `src/build_pattern/application.rs`, `src/build_pattern/daemon.rs`, `src/event.rs`, `src/lib.rs`, `src/multi_window.rs` | none | `Cargo.lock`, `Cargo.toml.orig` |
| Staged Niribar `vendor/layershellev` | 6 files | `src/events.rs`, `src/lib.rs`, `src/seat.rs` | none | `Cargo.lock`, `Cargo.toml.orig` |

Notify's initial vendoring is recorded in local commit `1ac419c72b3dc939fd5fe191817188f28174154d` on 2026-06-27, titled “pass the banner surface when using xdg-activation”; subsequent changes include `2284945` for banner output lifetimes. These are **Notify project commits**, not upstream revisions. Niribar's relevant local history includes `3f8f3030cc67410964c909df168f32608316789f` (2026-05-02) and `3cfbedf8dbee110c8ebc5fae67cb3e4947babe0d` (2026-06-11); those likewise identify host changes, not the crate release's provenance.

## Root dual-license text sources

Notify's root [Cargo.toml](../Cargo.toml) already declares `MIT OR Apache-2.0`. Preserve that expression for project-owned code. It does not make upstream MIT-only vendor code dual-licensed. Suitable authoritative text sources are:

- `LICENSE-MIT`: the [Open Source Initiative MIT text](https://opensource.org/license/mit), with an attribution scoped to this project's own authors/contributors. Do not substitute the upstream DecodeTalkers notice as the ownership notice for Notify itself.
- `LICENSE-APACHE`: the Apache Software Foundation's [plain-text Apache License 2.0](https://www.apache.org/licenses/LICENSE-2.0.txt). Preserve the standard terms and appendix; its sample appendix placeholders are part of the canonical license text, not unidentified vendor ownership.

A root third-party attribution/provenance document should separately name the two vendored crates, their MIT license, the original DecodeTalkers notice, and the local modifications. Copy their complete license files into release artifacts alongside the root dual-license texts. This is a concrete documentation recommendation based on the inspected metadata and exact license texts; it does not infer new licensing choices for contributors.

## Reproducibility inputs left in this session

- Existing Cargo artifacts: `/home/kevinard/.local/share/cargo/registry/cache/index.crates.io-1949cf8c6b5b557f/{iced_layershell,layershellev}-0.18.1.crate`.
- Downloaded artifacts: `/tmp/canopa-iced_layershell-0.18.0.crate`, `/tmp/canopa-layershellev-0.18.0.crate`.
- Downloaded upstream tag archives: `/tmp/canopa-upstream-v0.18.0.tar.gz`, `/tmp/canopa-upstream-v0.18.1.tar.gz`.
- License member paths: `exwlshelleventloop-0.18.0/LICENSE` and `exwlshelleventloop-0.18.1/LICENSE` respectively.

All archive comparisons used Python `tarfile` reads and byte equality without extracting or modifying the vendor trees. The temporary downloads support this investigation; permanent provenance should use the verified URLs, commits, and checksums above rather than depending on `/tmp`.
