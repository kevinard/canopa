# Niri remote input candidate

Registered, unadvertised keyboard transport candidate, 2026-10-31. The explicit
`niri-remote-input` recipe includes the exact `niri-desktop-compat` base before
these changes. The original GPL Niri and MIT Smithay notices remain
in their recorded recipes; the derived input backend also retains Smithay's full
MIT notice. No new dependency or lockfile change is required.

`org.canopa.Niri.RemoteInput1.OpenKeyboard` returns a socket lease, the configured
XKB keymap and authoritative depressed/latched/locked/layout masks. Input is eight
little-endian bytes: evdev key code followed by state 0 or 1. Output is a stream
of sixteen-byte modifier updates, four little-endian u32 values in that mask
order. The client must handle partial reads and retire on truncated output/EOF.
Closing the socket, malformed input or a changed keymap releases that source's
keys. Cleanup bypasses release-triggered compositor bindings and preserves keys
held by another source. KeyboardSnapshot reads authoritative state while the
client is unfocused. Consent and the frontend's active session remain portal-owned.

Admission is bounded to eight pending/active leases and eight pending D-Bus
commands, each lease to 64 held keys, each dispatch to 32 fixed frames, and the
keymap to less than 1 MiB. Evdev duplicate presses and unpaired releases revoke the lease.
The compositor refresh publishes modifier changes and retires changed keymaps
without another remote input request. A small Smithay getter exposes the already
cached keymap content identity; ordinary refresh never serializes the keymap.
Slow output readers are retired rather than accumulating an application queue.

`OpenKeyboardSymbols`, negotiated through `SymbolVersion=1`, separately accepts
four little-endian u32s: sequence, kind, value and state. Sequences start at1 and
never wrap; kind0 is evdev and kind1 is XKB keysym. Every command returns six u32s:
sequence, status and the four authoritative modifier masks. Status0 accepts,
1 refuses an invalid logical transition,2 an unavailable symbol,3 a conflicting
held base key and4 the held-key limit. Sequence0/status0 is unsolicited modifier
feedback. Logical refusal preserves prior held state; malformed framing revokes.
The original evdev method and byte format remain unchanged.

The compositor resolves symbols using its actual keymap, Caps and group in the
input callback. The planner borrows that map while locked, retaining only numeric
chords and references. Each symbol owns its required momentary modifiers until
release, even after feedback includes another held symbol's modifiers. Locks and
another source's keys are preserved; conflicting/unavailable symbols are refused.
Both protocols share admission and dispatch limits. Typed commands can expand to
at most six transitions, with the same64-physical-key limit and bounded writes.

The portal's EIS worker and Iced combined consent/controls have scoped native
acceptance. Legacy Notify wiring, pointer/touch, physical-input and distribution
acceptance remain. Do not install/select this compositor or advertise
RemoteDesktop from this candidate.
The original keyboard candidate passes nine checks. The state extension builds
locked/offline and passes thirteen private native checks, including Caps Lock,
French/US layout switching, asynchronous modifiers and idle keymap-change cleanup.
The typed extension passes sixteen model/wire tests, twelve native symbol groups,
all thirteen evdev regressions and six combined public-frontend/media/EIS groups
on one frozen compositor. These counts establish their recorded scopes only.
See the portal ledger for exact binaries, patch hashes and remaining acceptance.

Root provenance records both patches and exact file hashes, including explicit
null upstream hashes for four added files. Archive verification requires those
paths to be absent; existing files cannot be relabelled as additions. The generic
source checker and three addition/scope regressions verify this policy.

Prepare without installing or building:

```sh
python3 scripts/build-niri-screencast.py --overlay niri-remote-input \
  --archives ARCHIVES --work-dir target/NEW-SOURCES --prepare-only
```

The native matrix finished with its original tooling hashes unchanged; its149
fixture sources are preserved before this registration. Existing recipes remain
independently selectable. candidate.json retains the investigation/build history.
