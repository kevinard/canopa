# Real sandbox FileChooser and Documents integration

Phase82 extends the original FileChooser verification with a real private
Flatpak app and the standard Documents portal. Print remains deferred; this
reuses only its earlier bounded Flatpak tools/store/runtime, not Print requests
or failure-hardening work. No Canopa Rust/vendor code changed or Cargo build ran.

Current app: `org.canopa.PortalFileChooserProbe`, built from
`scripts/desktop-fixture/file-chooser-flatpak.c` against GIO, with no GTK UI.
Existing runtime: `org.canopa.PortalPrintRuntime/x86_64/1`, private store at
`target/portal-print-74`. No runtime files, SDK or system/user installation were
added. One tiny app ref was added to that fixture-owned installation.

Reproduce preparation using the same store and current build receipt:

```sh
python3 scripts/prepare-file-chooser-flatpak.py \
  --store target/portal-print-74 \
  --runtime org.canopa.PortalPrintRuntime/x86_64/1 \
  --client-build target/portal-filechooser-flatpak-82/client-build-02.json \
  --output target/portal-filechooser-flatpak-82/NEW-PREPARE.json
```

Omit `--client-build` to compile the small C probe into a stable output directory
whose `chooser-probe` does not already exist. This helper requires an existing
owned runtime, private tools and local `canopa-fixture` remote. It never downloads
them. Native verification takes the resulting client-build path:

```sh
python3 scripts/test-file-chooser-flatpak.py \
  --binary target/debug/xdg-desktop-portal-canopa --binary-sha256 BACKEND_SHA256 \
  --niri FROZEN_NIRI --niri-sha256 NIRI_SHA256 \
  --frontend FROZEN_FRONTEND --frontend-sha256 FRONTEND_SHA256 \
  --flatpak-store target/portal-print-74 \
  --client-build target/portal-filechooser-flatpak-82/client-build-02.json \
  --output target/portal-filechooser-flatpak-82/NEW-NATIVE
```

The normal backend is9b2ce29c; Niri92294c79 and frontend277568bf retain their
checksum-addressed stores. Full checksums are pinned in each native receipt.
The Documents/PermissionStore binaries are the installed standard services,
each pinned by checksum and started only on the private bus/data directories.

Evidence under `target/portal-filechooser-flatpak-82/`:

- `documents-capability-01/summary.json`: real owned FUSE mount, GetMountPoint,
  normal service teardown and unmount pass before app work.
- `native-dark-01`: failed fixture coordinates, leaving selection empty. Exact
  source/screenshot retained; no access grant observed. Do not relabel as pass.
- `native-dark-02`: genuine Flatpak identity and approval reached a Documents URI,
  but the fixture's custom `/tmp` runtime path was absent in the sandbox.
- `native-dark-03`: private runtime mapping corrected that path; launcher failed
  because ambient capabilities from the setup namespace reached bubblewrap.
- `native-dark-04`: ordinary launcher capabilities restored. Real Open/Save
  succeed; the old probe incorrectly rejected cancellation's empty URI array.
- `client-build-02.json` and `app-deployment-02.json`: only the C probe changed
  to accept an absent or empty URI list for cancellation; runtime unchanged.
- `native-dark-05/summary.json`: all four final groups pass: Open/read-only,
  Save, Cancel, and authenticated app ID/original preservation. Exact source and
  screenshots retained. Both FUSE and runtime bind unmounted, private path gone,
  every directly owned process terminal, all pinned inputs unchanged.

The runner preserves the invoking UID in a private user/mount namespace, binds
the temporary runtime onto its own `/run/user/UID` view, and drops inheritable/
ambient capabilities for the Flatpak launcher. The host runtime is never changed.
The original file is inaccessible before approval; only a `/doc/` URI gives access.
Save's bytes reach the selected original host directory through the granted alias.
The Iced chooser's mappings contain no GTK library. No document URI is returned
on cancellation. These are real sandbox/portal/FUSE checks, not a Registry-only
identity simulation or broad `--filesystem` access.

Scope remains the owned minimal GIO runtime at dark/100%. Standard GTK/Qt runtime
and application coverage, physical devices and broad distribution acceptance are
separate. Keep storage below128MiB across the shared store/new fixture inputs and
leave3GiB free. Do not create another SDK or repeat completed Print matrices.
