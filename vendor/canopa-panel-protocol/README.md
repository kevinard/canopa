# Canopa panel protocol snapshot

Canonical source: `canopa/notify/crates/canopa-panel-protocol`, protocol version 3.
This crate is a verbatim source snapshot, not a separately maintained protocol implementation.
It is vendored so a standalone Niribar checkout builds without a sibling Notify checkout.

After changing the canonical crate, synchronize `Cargo.toml` and `src/lib.rs`, then run
`bash scripts/check-panel-protocol.sh /path/to/niribar` from the Notify checkout.
Run Niribar's `canopa_provider` and `provider_protocol` tests after synchronization.
Do not evolve this copy independently. Additive host adapters belong in `src/external_panel/`.
