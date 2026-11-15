# Canopa Files

`canopa-files` is Canopa's file manager: column, list and icon browsing built for
tiled windows, floating Quick Look, tabs, search with filters, Recent, archives,
bulk Rename, devices and GVFS network locations, custom actions, and copy, move and
Trash operations with a persistent Undo and a Recovery store. The user interface uses
Iced and the shared `canopa-ui` controls. The same process serves the standard
`org.freedesktop.FileManager1` D-Bus interface.

- User guide: [docs/files.md](../../docs/files.md)
- Architecture and design decisions: [docs/architecture/files.md](../../docs/architecture/files.md)
- Glossary: [docs/files/CONTEXT.md](../../docs/files/CONTEXT.md)
- Configuration file: [docs/configuration.md](../../docs/configuration.md)

## Build and run

The binary needs the default `desktop` feature (Iced, GStreamer, GIO/GLib). Build
through the workspace's guarded Cargo wrapper, from the workspace root:

```sh
bash scripts/cargo.sh run -p canopa-files -- ~/Downloads
```

`--no-default-features` builds the renderer-independent core only; `local-runtime`
adds the headless runtime without Iced. The default build needs `pkg-config` and the
GLib/GIO and GStreamer (core and base) development libraries. See
[docs/building.md](../../docs/building.md) for build profiles and the free-space guard.

## Tests

Run focused checks from the workspace root (see the root `AGENTS.md`):

```sh
bash scripts/check.sh --files                       # copy and fingerprint checks
bash scripts/check.sh --files --lib [FILTER]        # library unit tests
bash scripts/check.sh --files --test CASE [FILTER]  # an integration case, e.g. transfers
bash scripts/check.sh --files --test transfers --test undo_changed_copy
```

Integration cases are the files in `tests/`. They share one `integration` executable
and are selected by module; repeat `--test` to cover related cases. Direct Cargo
commands use `--test integration cases::CASE::`. Choose the cases that cover the
behavior you change: library and copy changes usually also need the transfer, Undo
and Recovery cases. Filesystem tests use private temporary directories only.
