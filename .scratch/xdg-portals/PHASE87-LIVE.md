# Phase87: real sandbox ScreenCast and preserved PipeWire access

Goal ACTIVE, Print deferred. The preceding status-only goal turn was no progress;
the current turn revalidated the checkpoint and implemented the next safe action.
No user input, SDK/runtime download, Cargo build or live selection is needed.

The real Flatpak client now exercises authenticated identity, normal Rust/Iced
window selection and a portal-provided media FD. Cancellation, Iced Stop and app
exit without Session.Close pass. Both positive cases produce the exact same
400×300 pattern; unrelated nodes remain hidden throughout capture. Stop denies
another remote; both positive producers and controls retire. The app cannot
connect directly to the private host PipeWire socket or private backend.

Native result: target/portal-sharing-flatpak-87/native-dark-05/summary.json.
Four groups pass (three client cases plus authenticated app-ID forwarding).
All owned processes retire, the private bind unmounts, the temporary tree is
removed and changed_inputs is empty. This is dark/100%, window-only GIO on
PipeWire1.6.10/WirePlumber0.5.18, not all toolkits/distributions/devices.
Runtime identities remain a30backend/6e3d Niri/277568 frontend from phase86.
No application/compositor/frontend source or binary rebuild occurred.

The stock WirePlumber0.5.18 portal permission manager widens the frontend's
denied fallback after restriction. native-dark-03 is the failed baseline: real
pixels arrive but unrelated nodes become visible. The new original Canopa Lua
policy selects denied fallback/core-rwx for daemon-authenticated portal clients
with empty media roles, retaining frontend per-node grants. Camera/direct clients
and explicit administrator access policy retain their own handling. The optional
package includes the Lua/config files and refuses an incomplete policy component.
Five Lua boundary groups pass (policy-check-01.log); actual future PipeWire gated
handoff is unit-only, not a claim about an untested runtime. See ADR0112.

Initial failures stay failed: native-dark-01 registry exposure, diagnostic02
missing client adapter, native-dark-03 later exposure, native-dark-04 invalid Lua
permission string. Final uses Perm.NONE. The immutable preexisting runtime commit
394e7596f1d132f35bccd35338c60360f3f598b056e022a681fd932c6dff1e31 is unchanged.
Six installed PipeWire libraries live only in the tiny probe app; no SDK download
or runtime modification. Store+phase evidence allocation is135032832bytes, bounded
below136MiB, and root remains above3GiB. No cache deletion or per-case compositor
copy. Exact build/deploy: client-build.json and prepare-02.json.

Optional package regression: package-check-01.json,15groups PASS including missing
policy refusal and real relocated companions. Python AST and Lua syntax pass.
No broad workspace rerun for tooling/docs/policy-only edits; phase86 remains the
latest full workspace result. Do not replay its completed gate or Print findings.

## Terminal milestone

Delivery61408 and streaming verification21107 are TERMINAL0. All native, app
preparation, policy, package and syntax handles are terminal. No work is running.
Combined authoritative receipt: target/portal-sharing-flatpak-87/result-01.json.
The existing single development archive is refreshed in place. Full smoke with
desktop-sharing/extra requirements passes, including41offline docs/92links. The
streaming check matches all8applications+2companions to phase86 accepted identities
and verifies both new policy files against their manifest/source bytes. Both
updated setup/sandbox guides are present; no extra archive extraction is retained.

Archive: target/portal-release-package-check/canopa-x86_64-unknown-linux-gnu.tar.gz
Bytes:209521341; SHA256:4bfa111df9d8d9a738979114acb6f7fa4591731783439433151194e63caf05c0.
Receipt delivery-01.json records package91.43s/smoke10.63s and temporary parent
removal. delivery-verification-01.json records exact members. Comparison against
phase86's guarded crate/Cargo/non-Markdown vendor inputs finds ZERO changes;
runtime-source-comparison-01.json preserves it. No workspace replay is needed.

storage-cleanup-01.json audits every native case: private paths/binds gone, all
recorded direct PIDs absent. Combined fixture/store allocation135090176bytes,
4321280bytes growth from the preexisting store; root free3664986112bytes. No cache
deletion, SDK/runtime download, new target, persistent staging or binary snapshots.

This slice is DONE. Do not replay the sandbox cases, package, smoke or phase86
workspace/native matrices merely for continuation. The next goal turn should
reconcile original PRD/map requirements with current code and delivery, identifying
actual remaining implementation work before inventing further validation gates.
Keep Print deferred; unsupported touch, synthetic USB and scoped client/runtime
coverage stay explicit. Full goal ACTIVE, no missing user input or live changes.
