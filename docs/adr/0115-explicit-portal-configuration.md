# 0115: Generate explicit portal configuration without changing the session

Status: implemented; focused and public frontend verification pass, 2026-11-05.

Portal configurations are selected as complete files. Adding a user override
containing only FileChooser can lose the distribution's selections for other
interfaces. A fragment also leaves users to merge keys manually. Provide
`canopa-portal-config` with the application bundle to generate a complete review
file from an explicitly chosen existing configuration.

The user selects the base file. The utility does not guess the frontend's
build-time search directories or inspect/change the current session. By default,
select Settings and FileChooser. Repeated --interface options choose exactly the
named interfaces instead. Read interfaces and backend IDs from the bundle's
Canopa descriptors; Notification routes to canopa-notify, rather than canopa.
Optional sharing/input interfaces are available only when their descriptors
advertise them. Print is not added to the normal descriptors.

Prefix selected routes with the matching Canopa backend, followed by the existing
explicit backend list or the configured default. Preserve fallback order while
removing duplicate backend IDs. Keep default, unselected interface entries,
comments and unrelated sections unchanged. Respect effective last-key semantics
across repeated groups and preserve existing line endings. Reject syntax that
cannot be safely preserved, rather than dropping it silently. Source and
descriptor reads are bounded to 256 KiB and require UTF-8 without NUL.

Phase91 corrects a special-value error: `none` vetoes the whole backend list.
For an explicitly selected route containing `none`, replace the disabled list
with only Canopa; do not revive any inactive backend names. Unselected routes
and defaults containing `none` are preserved. The original phase90 frontend
fixture missed this because XDG_DESKTOP_PORTAL_DIR redirected its configuration
lookup away from the generated file. It now puts the file in the actual search
directory and asserts that the frontend reports loading that exact path.

Default output is standard output. --output creates one new mode 0600 file and
refuses existing files, source replacement and symlinks. It performs no install,
service reload, compositor restart or routing change. The resulting file is
reviewed before installation using the existing selection instructions. Retaining
fallback entries does not promise recovery from a backend that is advertised but
fails after selection. Metadata availability does not prove live services or
compositor support; optional companions still require their documented setup.

The helper uses Python 3's standard library, like the existing packaging tools;
portal implementations and interactive surfaces remain Rust/Iced. Keeping this
offline operation out of the backend avoids adding a configuration editor to an
on-demand permission service. The helper finds descriptors relative to its
installation prefix, and --portal-dir supports explicit layouts. It needs neither
GTK nor a running desktop bus.

Focused tests cover fallback preservation, separate Notify routing, repeated
groups/keys, CRLF/no-final-newline, malformed input, unsupported interfaces,
exclusive output creation and relocation. The private Settings frontend fixture
consumes a generated configuration and checks real activation, public values and
reload. Package smoke executes the relocated helper against the shipped
descriptors and verifies that ScreenCast/Print selections are preserved.

Current checks: 12 generator regressions and the retained 15 check-script tests pass. The real
private frontend consumes generated CRLF configuration with repeated groups/keys
and passes activation, public reads and reload signals. Host cleanup finds no
remaining private processes or directories. This is tooling-only work: all 1357
guarded Rust/Cargo/vendor runtime inputs match the accepted prior state. No new
Cargo build, runtime download or live selection change is required.

Selection semantics follow the upstream
[portals.conf reference](https://flatpak.github.io/xdg-desktop-portal/docs/portals.conf.html).
