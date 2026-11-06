# Draft live capture identities and metadata

Status: unadopted compositor foundation, 2026-10-31. This five-file Niri delta
extends the preserved pointer lease draft. Canonical vendor recipes, portal
consent and installed services remain unchanged by this slice.

Selection pins the actual live output object or unique window ID before stream
creation. A new output with the same name cannot revive a retired selection.
CanopaSourceVersion=1 and MonitorInfo expose the live output identity; optional
canopa-source-id on RecordMonitor/RecordWindow rejects stale selections. Existing
Mutter method signatures remain compatible. Separate bounded source queries keep
the existing reliable lifecycle channel for Stop and Drop.

Stream Parameters reads live logical geometry on demand, with a stable per-stream
mapping-id and source identity. Monitor scale changes and committed window resize
update size. Window position remains zero; this is not global input projection.
The existing dynamic picker retains its preselection placeholder and supplies no
absolute mapping. Start now marks a stream started atomically. Session Stop and
concurrent Record calls cannot leave a returned stream object live.

The preserved baseline reproduces the old 1×1 window and stale monitor geometry.
Four tests using actual Smithay Output objects pass, including concurrent clones
and same-name replacement. The first concurrent test exposed thread-local user
data insertion; the corrected implementation uses shared insertion. Failed and
passing receipts remain in target/capture-source-metadata-04/.

The frozen candidate passes 15 private metadata/lifetime checks in 4.28 seconds,
including the 64-pin limit, recovery, stale IDs, concurrent Record/Stop and window
replacement. The existing frozen Rust/Iced portal passes 17 real video groups at
125% scale in 88.58 seconds: monitor/window pixels, cursor modes, static resume,
sharing controls and caller/backend loss. All owned fixture processes terminated;
binary and fixture pins were unchanged. The incremental dev build took 190.84
seconds with 5,959 source inputs unchanged.

Absolute projection, capture-bound pointer transport, portal source catalog
negotiation, EI/Notify integration and pointer consent still remain. This is
scoped native evidence, not canonical recipe adoption, full workspace/matrix or
distribution acceptance. The full 18-interface roadmap remains active.
