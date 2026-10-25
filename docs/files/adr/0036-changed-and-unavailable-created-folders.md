# 0036: Review changed and unavailable created-folder locations

Status: accepted under delegated implementation authority; verified for the scope below.

A created folder can be moved elsewhere or replaced by another file, folder,
link or special node. Undo now shows the current item's real metadata and offers
Keep item, Skip and Cancel. Keep preserves both the shown node and any original
folder elsewhere, and finishes this created-folder step. It uses the existing
atomic created-container Keep transaction but grants no ownership of the
replacement to older history. Skip is the Return-key default. Links retain raw
Unix target bytes; special nodes are not opened as streams.

A captured metadata version supplies the actual physical identity used to
classify the location against that step's stable expectation. Only an owned
directory is inspected for emptiness. A changed node, including an empty
replacement directory, requires review. An owned empty folder retains its normal
identity-checked atomic removal; a nonempty one retains ADR 0035's choices.
Keep requires the shown version to remain unchanged and honours cancellation.

When metadata, entry lookup or directory inspection is unavailable, Undo shows
the recorded path and actual lookup/read error. Skip and Cancel retain the step.
Finish this step leaves files untouched and dismisses only that folder's removal
step. Created ancestor steps remain in history and are protected from earlier
in-batch namespace mutations until a fresh Undo. This avoids reporting missing
ancestors as kept or silently choosing their fate.

Finish must observe the same failure kind and metadata witness, when available,
after the answer. Reappearance, a different node, changed permissions/metadata or
a changed lookup failure requires fresh review. Unavailable children still
protect their ancestors during the current batch. Protected Recovery data inside
such a folder survives Finish, restored access, reopen, expiry and copy-out.

Outcomes now report folders not removed through `unremoved`, separately from
kept items, moved items not returned and previous versions not restored. A Finish
adds zero reversed changes. Operations uses distinct folder-removal counts and
details, including for partial batches with retryable ancestors. These source
and outcome fields are in memory only; history remains schema 8.

A regression also showed noninteractive Undo returning an error yet consuming a
missing-folder step through generic journal recovery. Created-folder inspection
now precedes creation of its Undo intent for both interactive and automatic
callers. Automatic reversal proceeds only for an owned empty directory; failures
and review states preserve the unstarted step. Durable intents from actual
interrupted removals retain their existing reconciliation path.

Seven new public parent tests cover replacement types, unavailable metadata and
read access, the noninteractive error, older ownership after Keep, stale choices,
ancestor retry and protected Recovery. Together with ADR 0035's five tests, the
focused folder suite passes twelve tests. Exact final proof is recorded in
`created-location-verification.json`. The native case
selector retains the owned nonempty-folder workflows and adds changed/unavailable
locations in both maintained themes, including truthful outcome counts.

This does not complete directory conflicts/Merge, broader batch decisions,
interrupted-directory recovery or original-media/directory-descriptor authority.
Those and the other product-brief work remain part of the active full goal.

Final core validation passes 190 tests with thirteen parent-invoked child
helpers. Build and all-target Clippy with warnings denied pass. Native 59 dark
and 60 light at 1.25 scale each pass nineteen checks; the original nonempty-folder
regression 61 passes fourteen, totaling 52 on the same schema-8 binary. All have
clean GTK logs. Four rendered replacement/unavailable dialogs were inspected.
Formatting, Python syntax, maintained inventory and whitespace checks pass. The
three initial red regressions and all focused, core, build and native logs remain
separate from final proof. No native fixture correction was needed this turn.
