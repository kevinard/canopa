# Production pointer consent, Notify and EI handoff

Status: scoped experimental production integration accepted, 2026-10-31. The
broader 18-interface goal remains active. Normal RemoteDesktop registration,
shipped advertisement and installed portal routing are unchanged.

Canopa's production RemoteDesktop presenter now offers separate unchecked Iced
keyboard and pointer permissions. Source selection grants no control. The backend
supports view-only, pointer-only and combined sessions, plus input-only pointer
control. Ongoing controls identify the approved devices and own a common stop
action. Runtime negotiation uses the original input owner: relative pointer
control requires PointerVersion 1, while captured input also requires the atomic
MappedPointerVersion 2 descriptors. Approval rechecks owner/capabilities and
requires complete capture bindings with known scale before Start succeeds.

The ordered public Notify queue now reaches the private pointer worker. Smooth
finish delivers its final nonzero delta before stopping every active axis. Zero
deltas and zero discrete steps do not synthesize stops. The adapter preserves
paired buttons and approved source coordinates. Keyboard and pointer transports
open lazily. Their failures retire capture, controls and the other input source.

ConnectToEIS detaches intentional-transition monitors and waits for both legacy
sources to retire before opening the bound EI worker. Held input is released, the
original video/mapping stays alive, and subsequent Notify calls are rejected.
The EI FD is transferred without retaining a duplicate; the session retains its
worker lease and waits for cleanup on closure.

| Final scoped check | Result |
| --- | --- |
| Default-feature incremental portal build | Passed, existing root target cache |
| Complete portal library | 160 passed, 5 ignored |
| Complete portal integration | 43 passed, 2 ignored |
| Portal Clippy and formatting | Passed |
| Public pointer/consent/media fixture | 17 groups per appearance; 34 executions |
| Existing public keyboard owner-guard and EI-handoff regressions | 2 groups per appearance; 4 executions |
| Actual window video | 14 frames through public restricted PipeWire FDs |
| Visual review | Fresh/selected consent and active controls inspected in both appearances |
| Patch replay | All 11 changed files matched their recorded baselines |
| Owned private native processes/workers | Terminal; none left running |

The pointer fixture uses a private descriptor, the pinned standard frontend and
nested Niri. Both dark/125% and light/125% runs verify fresh view-only selection;
separate pointer and combined approval; actual absolute/fractional relative
motion; smooth final-delta/finish ordering; discrete scrolling and zero steps;
legacy source death and explicit closure with held input; the same stream and
mapping before/after EI handoff; held-button/keyboard cleanup; and relative-only
Notify/EI without capture or keyboard. The 14 public video frames contain the
expected four colors at 500×375 physical pixels for 400×300 logical regions.
Producer removal is serial-aware. The keyboard regressions retain foreign-owner
denial and keyboard-only legacy-to-EI behavior with the new runtime device mask.

The initial final check sequence passes in 229.55 seconds. After visual fixture
adjustments, the unchanged Rust targets pass again in 27.44 seconds using the
same cache/profile/features. The final pointer bodies take 42.60 and 42.79 seconds;
their full pinned runners take 49.23 and 49.50 seconds. The keyboard runners take
28.54 and 28.52 seconds. All 1,322 inputs stay unchanged during each respective
check/run. The binaries are portal 1a221a6d4847d06d08a5f1d04d1689cdacbb1b2acdaaca1da02abb514408bcaf
and harness 31eb7007c20413b89742ea259a75e308f9d1093f16bfa261961620207cf0abec.
The existing scale-capable compositor and ordered frontend remain pinned to
84acceed40ace2be1362ecfd32239b0bc58b909f070801144a3e0962952168c9 and
b77492eb34753d19901f1d0825cbb226a61c2436474445718504e850995d33b3.

Earlier results are retained: the first native fixture expected AccessDenied
where the pinned frontend returns Failed with its precise not-allowed message.
Another waited for Closed after explicit Close; pinned xdp-session.c calls
xdp_session_close(session, FALSE), so producer/control/input retirement is the
appropriate barrier. Initial Clippy rejected two unnecessary unwraps, replaced
with structured ownership matching. Visual review found a premature selected
state capture and a fixture-positioned control panel extending past the display;
the final fixture waits for rendering and keeps it within bounds. These were not
counted as final acceptance.

[evidence.json](evidence.json) records scope, hashes, counts and binary archives.
[portal.patch](portal.patch), copied baselines/current sources and
[the replay log](replay.log) preserve the delta. Final native helper sources,
consent/control PNGs and video frames are saved under native-proof/. Full logs
live at target/portal-pointer-production-11/. Native binaries were frozen in
/tmp to preserve repository build space, then losslessly archived under
target/portal-pointer-production-11/binary-archives/; decompression hashes match
the executed ELF files. Build outputs and caches were not moved or cleaned.

Remaining acceptance includes public monitor/multiple-source and mutable
projection/loss/race cases, physical input coexistence, touch, compositor
availability changes at the frontend, canonical companion adoption/distribution,
and earlier full-workspace/native failures. No vendor or installed-service source
changes belong to this slice. See [ADR0053](../../../docs/adr/0053-pointer-consent-and-production-input.md).
