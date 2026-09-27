# Private control API over Unix socket

Notify exposes its private control API over a per-user Unix domain socket under the runtime directory. The freedesktop notification server interface remains D-Bus, while Niribar provider and CLI control traffic use local private IPC so the control protocol can evolve independently.
