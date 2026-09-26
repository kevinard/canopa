# Cargo workspace from the start

Notify starts as a Cargo workspace rather than a single crate. The daemon has independently testable boundaries for notification policy, storage, D-Bus receipt, Wayland presentation, private control API, and Niribar provider integration, so a workspace keeps GUI/runtime dependencies out of pure domain and protocol tests.
