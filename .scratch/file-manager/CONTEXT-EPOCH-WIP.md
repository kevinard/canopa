# Recovery context authority checkpoint

Historical ADR0048 snapshot. The proposed shared-owner tracer below subsequently
ran red/green and is verified under ADR0049; current work/evidence is recorded in
SHARED-RECOVERY-WIP.md and shared-recovery-verification.json.

Full file-manager goal is active. This is progress, not full product/Undo
completion. No agents or external blocker. Baseline main remains
65e7cc245dc52607196bae6330f4a92da49918e2; .git is read-only.

## Current change

Schema16 adds exact saved-container and owner step keys plus logical parent
identity to archived context markers. Source-folder context uses a scoped PARENT
expectation; active file context uses its PRIMARY ownership. Reverse requires
the exact owning step and container reversal pops only its own marker. v15 bare
path maps become a single qualified recorded-location hint with no inferred
authority. Recovery still follows and previews actual saved data; its copy
chooser avoids choosing an unproven original parent.

`store/origins.rs` indexes marker keys and queues only affected archives on step
deletion. Trim/startup retire dead markers in256-entry pages, retaining a live
newer owner after an older container key expires. Existing transaction ownership
and physical saved identities remain the authority for files.

## Feedback so far

- context-epoch-red.log: unrelated later file Undo incorrectly rewinds archived
  origin B→A, while actual saved data remains intact. Red session72666/101.
- context-epoch-first-fix.log: first exact-step fix passes.68172/0.
- context-parent-epoch-red.log: a recreated source folder at A claims the older
  partial archive.2102/101.
- context-epoch-parent-fixed.log: parent identity and active file ownership pass
  four context cases.67185/0.
- context-epoch-focused-first.log: five upgrade parents and21 saved-transfer
  parents pass, one parent-invoked ignored helper definition. Cargo host731798
  and test732457 observed live after context compaction, then terminal; all
  focused tests finish74.75s. No duplicate run started due lost observation.
- context-epoch-path-reuse.log: six context parents pass, including explicit
  saved-box movement followed by the same containing-folder path pair and
  reversal.45812/0; the earlier compile-only E0164 was corrected.
- context-epoch-active-alias.log: renamed folder + crossFS Merge/KeepBoth has
  correct active Recovery alias, but older replacement Copy Undo asks for
  changed-copy review.6998/101.
- context-epoch-active-alias-probe.log: tagged temporary instrumentation confirms
  expected old inode vs recreated actual inode; fingerprint matches and public
  file assertions confirm right name and bytes after both returns.15720/101.
  The debug line has been removed from source.

The source ownership fix follows committed Rename/Trash/Restore namespaces from
each older reference with the same expected physical identity. Historical paths
stay attached to Undo; matching an inode elsewhere is insufficient. The first
walk-all-changes version passes the alias parent (30001/0). It is now replaced
with paths::follow_namespace, a source-path-indexed lookup of only reference
ancestors' committed movements in order. Initial population pages256 changes.
references::create now runs for upgrades too, ensuring the new expected-identity
index exists on prior schemas. Authentic old fixtures remove these new derived
structures as well. Temporary debug instrumentation was removed.

## Verification

90660 terminal0: full core282 parents pass,19 parent-invoked helper definitions;
log context-epoch-final-core.log. New indexed-source alias regression passes in
that run, alongside all other ownership tests. 75998 static matrix/syntax/
whitespace/source snapshot pipeline terminal0; runner preflight58538 terminal0.
Compiled manifest context-epoch-compiled-source.json has98 Rust/build paths.
74003 terminal0: Clippy53.03s, default build44.94s, format5.72s,98 compiled source
paths unchanged. Desktop SHA
2ae680c7f0229f85f0d051bcd685b27c067d2598d10cb09528908c4af4659c59.
Build-state JSON records actual commands/times.84020 terminal0:150 tracked and
untracked source paths whitespace clean. No Rust edits after the snapshot.
Active alias first fix30001 passed; compile-only inference mistakes corrected.

Native97 dark/46980 terminal0:13 checks pass; actual devices35/2050; private
secondary removed. All8 captures inspected at full size: origin B before/after
unrelated Undo, Recorded location qualifier after actual15 startup upgrade and
after KeepBoth return, three saved-text previews and the copy chooser. Both
files logs clean. Source bytes and recreated source work are preserved.
Native98 light1.25/49829 terminal1 after successful KeepBoth filesystem checks,
at waiting for Recovery compositor focus. Stable existing Recovery id9 remains
open; browser is focused. All5 earlier captures inspected; failure.png inspected;
no application warnings or data failure. Native fixture previously assumed a
remote gapplication action obtains focus, which a remote action without the
injected user's activation token does not establish. Keep any delayed modal
focus restoration vs compositor policy cause distinction unclaimed. The helper
now selects the existing Recovery tile explicitly before keyboard input, without
changing app code or data assertions. Broader remote-activation focus acceptance
remains separate required work. Original fixture hash and97/98 outcomes retained
in context-epoch-native-first-fixture.json. Both private destinations removed.
Native99 light1.25/78852 terminal0 with corrected selection:13 checks,35/2050
devices, both logs clean, secondary removed. All8 captures inspected via contact
sheet plus full-size returned qualifier/chooser; previews show protected edits
and original filename. Native100 dark/25771 terminal0 uses the same corrected
fixture:13 checks,35/2050 devices, both logs clean, secondary removed. All8
captures inspected via contact sheet plus full-size unrelated-origin, returned
qualifier and chooser captures. No Rust edits; core/build proof stays current.
20 Python scripts parse after the fixture edit; all98 compiled source hashes
still match. Both final runs total26 checks and16 inspected captures.

The older schema15 desktop f8b651... is historical proof only. Schema16 proof is
context-epoch-verification.json with context-epoch-source.json and the compiled
manifest. All sessions terminal. Full product goal stays active.

## Next

1. Run the public shared-owner tracer below before changing GC/disposal.
2. Finish unusual saved payloads/extra contents and directory Recovery, explicit
   private selection and already-decomposed legacy returns. Preserve unchanged
   source/data authority and interruption journals.
3. Continue the entire original ledger Next work list, including bounded catalog
   work, durable jobs, original-media/directory authority, remaining desktop and
   release integration. Do not mark the full goal complete. The new maintained
   native context case and its final proof need no rerun without a relevant change.

Raw SQL prepares authentic older-schema/malformed inputs only. Tests observe
public Engine/transfer/Undo/Recovery and actual private files, never DB outputs.
Preserve schema15 proof as historical: SAVED-TRANSFER-WIP.md and saved-transfer-
verification/source manifests. The false ancestor-heading finding remains closed.

## Next public shared-owner tracer candidate (not yet run)

`Store::collect_garbage` currently calls expire_retained for each archived record
without asking the existing stage-owner index whether a live Undo still needs
the same saved payload. Before changing it, prepare a valid older-schema shared-
owner input from an actual public Copy-replace: keep its active retained record,
add an archived unprotected record of that same saved container, then reopen.
Observe only public Recovery/Undo and actual bytes: startup/normal trim must not
delete the active backup; current destination stays unchanged; ordinary Undo can
still restore the older bytes. This is fixture input, not a private DB output
assertion. Rebuild/downgrade new16 derived structures consistently. Schema16's
origins upgrade already reindexes archived records through paths::index_record.
No data-loss reproduction or fix is claimed until this tracer runs red/green.

Then cover shared archived protected and unprotected owners. Automatic expiry
must not drop the protected record or unlink data another owner needs. If safe
expiry can discard only redundant unprotected metadata, separate its read-only
eligibility check from actual unlink; do not call the mutating expiry just to
decide. Keep changed-version checks, identified stages and journaled disposal.
Directory/special/additional saved payloads, explicit private-folder selection,
already-decomposed legacy handling and the entire ledger still need completion.
