//! Integration cases share one executable to avoid linking each source file.
//! Filter by module with `cargo test --test integration MODULE::`.

#[path = "support/daemon.rs"]
mod daemon_support;
mod support;
use support::notifications;

mod active_resources;
mod configuration_intake;
mod control_api;
mod control_resources;
mod control_spine;
mod dbus_contract;
mod dbus_receipt;
mod dbus_restart;
mod history;
mod input_limits;
mod legacy_import;
mod lifecycle;
mod mutation_completion;
mod notification_receipt;
mod packaging;
mod panel_provider;
mod policy;
mod preferences;
mod presentation;
mod provider_resources;
mod renderer_startup;
mod replacements;
mod service_lifecycle;
mod shutdown;
mod sound;
mod storage_failures;
mod summary;
mod summary_transport;
mod timer_resources;
mod transient_center;
