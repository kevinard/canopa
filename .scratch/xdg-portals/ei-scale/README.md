# Atomic scale for capture-bound input regions

Status: prepared compositor extension and Rust worker integration with scoped
acceptance, 2026-10-31. EI pointer devices and Iced permission remain incomplete;
runtime pointer grants and normal RemoteDesktop registration remain disabled.

The version-1 baseline reproduces the missing invariant: a window-only recording
keeps source identity, mapping ID and logical size 400×300 while its video changes
from 500×375 to 600×450 pixels. The held pointer source stays active. An immutable
EI region would need a new scale even though its size did not change.

The compositor now reads monitor scale from the real Output and window scale
from its actual cast output. Private Parameters optionally expose the double
canopa-physical-scale. MappedPointerVersion 2 atomically returns stream ID,
mapping ID, logical size and physical scale, and invalidates on any scale change
as well as existing identity/size changes. EOF follows retirement of that source's
held input. Relative pointer framing and keyboard behavior are unchanged.

The Rust capture binding carries optional scale with bounded validation. The
pointer worker accepts descriptor versions 1 and 2; missing version-1 scale stays
unknown, rather than defaulting to 1.0. Version 2 requires known valid scale. The
atomic Open descriptor is authoritative over a preceding metadata query. After
confirmed cleanup, renewal requires the same live source/mapping identity and a
verified size or known-scale change. Lost known scale and malformed projection
metadata fail closed. Held state and interrupted commands are never restored.

Public StreamMapping remains ID plus logical size; compositor object paths and
the new physical-scale field stay in the private recording/input boundary. EI
regions must later receive this snapshot and be replaced coherently on renewal.
No actual EI pointer device is introduced in this slice.

| Final scoped check | Result |
| --- | --- |
| Portal capture tests | 15 passed |
| Complete portal library | 153 passed, 5 ignored |
| Complete portal integration | 43 passed, 2 ignored |
| Portal Clippy and formatting | Passed |
| Compositor formatting and same-cache incremental dev build | Passed, 170.87 seconds |
| Window-only compositor scale regression | 3 native groups; identical test fails on old provider |
| Existing plain/padded mapped geometry | 32 native groups |
| Existing pointer, binding, drag and keyboard regressions | 94 native groups |
| Production Rust window-only capture and worker renewal | 7 native groups; three scale changes |
| New descriptor-v2 pointer/video/retirement | 11 native groups, 64 resize transitions |
| Existing descriptor-v1 compatibility | 11 native groups, 32 resize transitions |
| Public mapped provider, dark/125%, video and Iced controls | 17 native groups |
| Legacy provider, light/100%, view-only video | 17 native groups |
| Combined mapped video and keyboard EIS, light/125% | 6 native groups |

The 129 compositor and 69 portal native groups include actual image dimensions,
four captured color regions and client coordinates, empty held state after
renewal, repeated scale changes at constant logical size, pointer Stop/reopen,
backend death, selected-source loss and serial-aware producer cleanup. All
recorded helper/binary pins, 5,961 compositor inputs and 1,313 portal inputs stay
unchanged during their respective builds/runs. All owned native jobs terminate.

The successful compositor regression takes 5.26 seconds; existing compositor
regressions take 176.89 seconds; portal native suites take 306.70 seconds. Portal
checks retain the first run's 153 library successes and the final continuation's
43 integration successes, Clippy and formatting. The initial integration run
passes 42 tests and times out waiting for the existing backend-death helper to
start. Its cause remains unresolved. A direct run on the identical executable
passes in 0.33 seconds; the unchanged full rerun passes. The failure remains
evidence, not an explained or blanket flaky classification.

[evidence.json](evidence.json) pins the artifacts and exact scope.
[niri.patch](niri.patch) is the four-file delta on the mapped-pointer draft;
[portal.patch](portal.patch) is the six-file portal delta. Both have preserved
baselines and successful isolated replays. A 20-file layered replay also passes from the accepted canonical E5 base through
all ownership/lease/metadata/mapped/scale draft deltas. Full canonical archive
verification is still required before adoption. Execution artifacts are at
target/portal-ei-scale-09/. Native sources, receipts, logs and PNGs are retained
in native-proof/.

The compositor candidate is 84acceed…; its C56 predecessor remains unchanged.
Public pointer transport, EI region/device lifecycle and explicit unchecked Iced
permission are next. Remaining window activation/occlusion, physical input,
canonical compositor/provenance adoption, distribution, earlier full workspace/
native matrix failures and the full eighteen-interface goal remain open. See
[ADR0051](../../../docs/adr/0051-atomic-input-region-scale.md),
[pointer worker evidence](../pointer-workers/README.md) and
[the mapping ledger](../POINTER-MAPPING.md).
