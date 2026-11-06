//! Integration cases share one executable to avoid linking each source file.
//! Filter by module with `cargo test --test integration MODULE::`.

mod v5_sessions;
mod wire_compatibility;
