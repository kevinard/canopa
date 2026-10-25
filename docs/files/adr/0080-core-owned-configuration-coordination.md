# 0080: Core-owned configuration coordination

Status: accepted, scoped verification, 2026-10-19. Continues ADR0019/0060/0074/0076.
This is host-ownership work; resident startup and activation remain unfinished.

Before this extraction, the serialized configuration worker lived in the Iced
adapter. It owned
request admission, writes, external polling, publication and stop policy in
addition to mapping events onto views. Extract that complete coordination into
ConfigurationJobs beside the existing ConfigurationRuntime, with no renderer or
new dependency. Exercise the existing public configuration/reload seam using real
private source/theme files under ADR0001's delegated test-seam authority.

ConfigurationJobs owns a bounded request queue for Save, SaveSettings and Reload,
serialized with the existing250ms external-source polling. Preserve immutable
source/version checks, actual write acknowledgements and last-valid reload
behavior. Arbitrary source changes never execute actions. Worker clones share an
owning handle; dropping a frontend clone, notice receiver or reply receiver does
not cancel accepted work while the runtime retains its owner. Final-owner Drop
requests stop and never joins filesystem work on the caller's UI thread.

The coordinator retains its original discovery, explicit-path or defaults
selection. Reject a supplied base from another selection before reading,
writing or replacing the polling runtime. Stale bases from the same selection
still pass through the existing source/version checks. Discovery and symlink
retargeting retain their normal observation policy; an editor cannot redirect
the worker by supplying another loaded snapshot.

One bounded snapshot retains the current valid configuration, latest monotonic
publication and pending request count. Every publication carries the complete valid
candidate, its own candidate revision, and any rejection together. Reads do not
consume another frontend's latest state. A coalesced rejection cannot hide a valid
candidate that the frontend has not yet applied; presentation tracks publication
and applied-candidate revisions separately. Publish failed writes as rejections
while preserving the last valid configuration; a dropped reply cannot hide the
failure. Exact per-request one-shot replies remain separate from coalesced notice
wakeups. Source churn cannot queue an unbounded history of snapshots.

Create the owning service before Iced initialization and pass a clone to the
frontend. Its pending requests participate in application holds. Iced maps notice
streams/publications and owns editing drafts, focus and visible error treatment;
it no longer owns the configuration request loop or its cancellation lifetime.
A future host still needs orderly shutdown/admission policy across all services.
No durable draft replay or renderer-independent hosting completion is inferred.

Verify that real saves and external reloads advance without a frontend, failed
stale writes remain observable, source preservation and settings/action edits
coexist, acknowledgements survive an undrained notice stream, and existing native
action/preferences workflows retain their behavior. Record actual results in the
implementation ledger; these acceptance targets are not themselves passing proof.

## Scoped verification

Six public coordination tests and five reload tests pass on actual private
sources. They cover observer/frontend detachment, actual saves and external
reloads, failed stale writes, preserved source selection, exact replies and
coalesced complete publications. Forty-seven Iced library tests and both scoped
review axes pass. Default all-target Clippy, formatting and normal build pass.
The full core run passes 461 parent tests plus
2 isolated descriptor helpers.

Four maintained native action/preferences workflows pass 46 checks across both
themes, including light at 125% and narrow 360px forms. The first dark-action launch
timed out before a window appeared; its identical repeat passes. The original
failure remains recorded with no established cause, and neither timeout nor
fixture oracle changed. The isolated session logs contain only four expected
unavailable GIO remote-monitor messages each. See the implementation ledger and
configuration-runtime-verification.json for exact source/binary/run evidence.
This is configuration coordination acceptance, not resident-host or full-product
acceptance.
