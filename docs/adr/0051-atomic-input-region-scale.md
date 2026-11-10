# 0051: Atomic physical scale for input regions

Status: prepared compositor extension and Rust worker integration with scoped
acceptance, 2026-10-31. Canonical compositor adoption, EI pointer devices and
explicit Iced pointer grants remain outstanding.

A captured window can retain its logical size while its physical video scale
changes. The frozen version-1 provider reproduces a 400×300 selected window whose
video changes from 500×375 to 600×450 pixels at the same source/mapping identity.
Its old pointer lease stays active with a button held. Size-only invalidation is
therefore insufficient for immutable EI regions. This does not establish an EI
regression in a shipped pointer implementation; that path is still unadvertised.

Read physical scale from the compositor's actual output for monitor capture and
the window's cast output for window capture. Add optional canopa-physical-scale
double metadata to the private ScreenCast Parameters. Keep legacy view-only
metadata compatible. Source identity and mapping identity retain their meanings;
scale is mutable projection state, rather than a new permission or source.

MappedPointerVersion 2 returns physical scale alongside each stream ID, mapping
ID and logical size in OpenMappedPointer. Snapshot these together on the
compositor loop. Require a started producer, the exact selected session/stream/
target, valid dimensions and finite positive scale bounded at 64. The binding
compares scale as well as size and identities. A scale-only change retires the
old source and held state before EOF. The unbound relative-pointer and keyboard
contracts are unchanged.

The Rust recording worker carries optional scale through the private binding.
The pointer worker negotiates descriptor versions 1 and 2. Version 1 has unknown
scale; never reinterpret missing metadata as 1.0. Version 2 requires known valid
scale. Prefer the atomically returned descriptor to a preceding read-only query,
which can race a legitimate configuration change. Retain the original owner/PID,
source and mapping checks and FD peer validation.

After confirmed EOF, the worker may renew on a verified size or known-scale
change within the same binding. The replacement starts with empty held state and
does not replay interrupted input. Lost known scale, malformed metadata, identity
loss or unexplained unchanged projection ends the worker. This lease renewal
must later drive immutable EI device replacement; changing the private lease
alone does not implement EI device lifecycle.

The identical window-only native regression fails on the old provider and passes
three groups on the candidate. Its replacement scale matches actual video and
client coordinates and has no inherited held button. The production Rust capture/
pointer path passes seven groups, including three alternating scale changes with
unchanged logical size and producer identity. Version-2 and version-1 pointer
regressions, public mapped/legacy video and keyboard EIS/media also pass.

Scoped acceptance is 153 portal library tests, 43 integration tests, Clippy and
formatting; 129 compositor native groups and 69 portal native groups. The
compositor build retains 5,961 source inputs and the existing incremental dev
cache/compiler. Portal checks retain 1,313 inputs and normal incremental debug
settings. One integration helper-startup timeout remains unexplained; the same
executable's direct case and unchanged full rerun pass. Earlier workspace and
native matrix failures remain open.

The four-file compositor delta and six-file portal delta replay against their
preserved baselines. The compositor delta sits on the prior mapped-pointer
draft; it is not full source-archive verification or canonical recipe adoption.
Licenses and provenance must be adopted together with the complete companion
after remaining acceptance. No service installation or routing change is made.

The [EI region contract](https://libinput.pages.freedesktop.org/libei/interfaces/ei_device/index.html)
requires immutable region configuration and defines physical scale for relative
movement across regions. That contract motivated this prerequisite. Next work is
EI capability/region layout, per-device frames and held state, device replacement,
production legacy pointer transport, and fresh unchecked Iced consent.

Evidence: [scale ledger](../../.scratch/xdg-portals/ei-scale/README.md),
[public stream pairing](0050-pointer-notify-and-stream-pairing.md),
[private worker ownership](0049-capture-bound-pointer-workers.md).

Update, 2026-11-10: the source-overlay recipes were removed with `vendor/`; adopted compositor changes are branches of kevinard/niri, whose pinned `canopa/main` the packaged compositor is built from (`scripts/build-niri.py`). See ../vendor-maintenance.md#niri-compositor.
