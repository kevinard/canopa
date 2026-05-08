# Treat Niri as the First Compositor Integration

Niribar is designed for Niri first, but Niri IPC is not the runtime boundary for creating **Bar Surfaces**. Wayland provides **Eligible Outputs**, while compositor-specific integrations enrich dependent **Modules**; this keeps Niri-backed modules available when Niri is present without preventing future support for other compositors.
