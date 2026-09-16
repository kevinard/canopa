# AccessKit Unix local patch

Upstream: accesskit_unix 0.23.0, AccessKit commit
`42e53b0d829e7a0b34dc8803bb06012db2e80cc6`, `adapters/unix`.
Published crate SHA-256: `665c9079b7325a8168a6793437a172dda44b19a05af8ed52d7e32abaada996fe`.
The crate's `.cargo_vcs_info.json` identifies this revision. MIT OR Apache-2.0;
full license texts copied from that exact upstream revision. Original source
copyright/SPDX notices are preserved.

Local change: `src/atspi/bus.rs` wraps AddAccessible and RemoveAccessible payloads
in one-element tuples. Each AT-SPI cache signal carries one structure argument;
zbus otherwise flattens that structure into separate arguments. Required wire
signatures are `((so)(so)(so)iiassusau)` and `(so)`. Actual Orca50.2/libatspi rejected
the original retirement signal as `Unknown signature so for RemoveAccessible`.

Notify's native accessibility regression checks Hyperlink.GetURI and both cache
signatures using `.signature().to_string_no_parens()` (ordinary Signature string
equality normalizes the outer structure and cannot distinguish this bug). Repeat
that test, Niribar's native test, and real Orca replacement/retirement checks on
updates. Remove the local patch when an upstream release provides equivalent
behavior and those checks pass. The only source modifications are these two call
sites; Cargo.lock and Cargo's extraction marker are omitted from this snapshot.

Packaging-only normalization: remove the extra blank line at the end of
`Cargo.toml.orig`; the original manifest content is unchanged.
