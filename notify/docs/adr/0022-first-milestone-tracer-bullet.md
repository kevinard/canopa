# First milestone is a tracer bullet

Notify's first milestone is a vertical tracer bullet that receives a freedesktop notification, applies default-allow policy, stores it, shows a real minimal banner, exposes it through minimal control commands, and renders it through the Niribar panel provider path. It does not need user-authored rule parsing yet. This validates the real D-Bus, Wayland, SQLite, private IPC, and Niribar integration boundaries before expanding rules and UI depth.

Development runs may use a dev/test D-Bus service name so the tracer bullet can be exercised without taking over the user's normal `org.freedesktop.Notifications` service. Service-mode runs use the real freedesktop notification service name.

The receive/store/banner/control parts of the tracer bullet can proceed before shared panel protocol extraction. The Niribar panel-provider part waits for the shared protocol crate instead of copying protocol types locally.
