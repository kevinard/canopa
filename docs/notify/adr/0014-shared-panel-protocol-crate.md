# Shared panel protocol crate

Notify and Niribar share external panel protocol types through a small shared protocol crate rather than making Notify depend on the Niribar application crate. The protocol should be extracted from Niribar into a Canopa shared crate before the notification center provider slice depends on it. This keeps the dependency direction clean while allowing the notification center provider contract to remain versioned and type-safe.

The shared panel protocol crate is generic Niribar panel infrastructure. It must not contain Notify-specific notification logic. Notify owns **Notification Center Projection** in the Notification Daemon / Notify Control API implementation and maps it into the shared panel protocol through a Panel Provider Mode adapter.
