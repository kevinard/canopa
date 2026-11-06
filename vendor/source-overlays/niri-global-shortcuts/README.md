# Experimental compositor shortcut leases

Status: canonical private foundation. The current inhibition admission revision
passes its build, 53 actual-source core tests and 22 final public-frontend editor
groups. Native window lifetimes and all six shared desktop suites pass (ADR0068).
The full original workspace retains the same ten Files failures; all other Rust
targets and post stages pass. The preceding foundation
passed 50 scoped native groups; its workspace gate retained the same ten Files
failures, with all other Rust and post-Rust stages passing. This recipe retains
the validated niri-input-capture companion and
adds a private compositor shortcut owner. It does not implement or advertise
the public XDG GlobalShortcuts portal. Rust session ownership and Iced consent
and configuration are implemented experimentally in the Portal crate (ADR0067),
outside this compositor recipe.

The service org.canopa.Niri.GlobalShortcuts1 at
/org/canopa/Niri/GlobalShortcuts1 exposes OpenShortcuts() returning one Unix FD,
with read-only Version 1. That FD is a trusted desktop backend capability and
must not be passed to an application. The backend must verify the original
service owner, compositor process and version as for the other private leases.
At most sixteen pending/live leases are admitted.

Opening an empty trusted lease is allowed while a focused surface inhibits
keyboard shortcuts, so a second consent editor can open. Lock and active
compositor UI still prevent admission. Inhibition continues to suspend action
activation and keeps all registrations reserved; cancellation never requires
temporarily releasing their triggers.

Frames are a little-endian u32 byte length followed by JSON, at most 32 KiB.
Each request has a strictly increasing nonzero sequence and a command:

```json
{"sequence":1,"command":{"op":"list"}}
{"sequence":2,"command":{"op":"replace","generation":2,"bindings":[[1,"CTRL+a"],[2,"F9"]]}}
```

List returns the current generation and admitted bindings. Observe that generation
before Replace; initialization or revalidation can publish Changed before the
first command reply. Replace atomically installs the entire proposed set or
preserves the previous set, including active press recipients. Stale generation,
unresolved symbols, duplicate triggers and ordinary/a11y/other-lease conflicts
return explicit errors and failed action IDs where applicable. Configuration is
limited to 64 actions, 16 layout-qualified aliases per action and 512 slots per
lease. Num Lock is optional unless NUM is requested; Caps Lock does not alter a
base-key trigger. Unrecognized modifier bits do not silently match.

Activated/Deactivated events contain the generation, numeric binding ID and
source event timestamp in u64 milliseconds. No activation token is fabricated.
One accepted action activates once across repeats, aliases and overlapping
sources; the final holder ends it. Rebind, suspension or lease loss retires its
authority and retains suppression until actual seat-up. Source removal uses
the post-release seat state and cannot emit an unmatched ordinary key-up.
Physical devices receive distinct source identities; the bounded 256-device
table refuses further shortcut admission on exhaustion. Virtual and authorized
RemoteInput sources retain their existing distinct identities and ordinary
compositor policy.

Normal/hardcoded bindings and accessibility take precedence. Lock, shortcut
inhibition and active compositor UI suspend portal actions. Current per-key
layout and the ordinary ASCII fallback decide matching. Keymap/config/a11y
changes retire old generations before publishing the new admitted subset;
already-approved choices can recover when a temporary conflict disappears.
Layout group switches do not rebuild registrations or redirect a held release.
The existing accessibility monitor ABI and its separately recorded event-pairing
gap are unchanged.

Each calloop dispatch reads at most eight commands. The input thread never waits
for a reader: malformed/truncated commands, EOF or a blocked event write retire
that lease. Drop shuts the stream down even if a readiness descriptor remains.
The actual canonical core passes 53 tests and the final compositor passes 50
scoped native groups, including config/keymap reload, source withdrawal and
unread-output retirement. The required workspace, broader physical/layout and
distribution acceptance are tracked in the GlobalShortcuts ledger. Public Rust
sessions and Iced consent/configuration are experimental under ADR0067;
normal advertisement remains pending broader acceptance.

The complete Niri patch is relative to its official pinned archive, with the
existing canonical Smithay patch referenced from niri-input-capture. Niri's GPL
and Smithay's MIT notices remain referenced from niri-desktop-pointer. Exact old
and new archive materializations verify all inputs and normalize only the two
original Smithay dependency bindings. Reuse the stable source paths, existing
incremental dev cache, one job and recorded nightly compiler:

```sh
python3 scripts/build-niri-screencast.py --overlay niri-global-shortcuts \
  --archives ARCHIVES --work-dir PREPARED_SOURCES --reuse --toolchain nightly
```

This does not install or select a compositor or change live portal routing.
