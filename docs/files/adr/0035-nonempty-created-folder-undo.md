# 0035: Keep or skip a nonempty created folder during Undo

Status: accepted under delegated implementation authority; verified for the scope below.

Undo removes a folder it created only while the recorded folder still owns that
location and is empty. Files added afterwards must survive. Previously interactive
Undo failed with a generic nonempty-directory error, leaving the user without a
usable decision. It now shows the actual folder metadata and offers Keep folder,
Skip and Cancel. Skip is the Return-key default; Escape cancels remaining work.

Keep folder leaves the folder and all remaining contents in place and finishes
its created-folder step. Its created ancestor containers from the same operation
are kept in the same existing metadata transaction. Outcomes count these as kept
items, separately from changes actually undone. Other completed copy leaves can
already have been reversed before this folder is reached. Skip preserves the
folder step and its needed ancestor containers for later retry; when the user
moves new contents elsewhere, an owned empty folder can be removed normally.

The review observes the folder's actual physical identity and metadata version.
Keep refuses a replaced node or an observable directory change while the question
is open; cancellation also takes precedence over the answer. The ordinary
empty-folder reversal retains its identity check and atomic `remove_dir`, so a
late arriving child prevents removal. There is no recursive deletion option.
A bounded first-entry inspection runs on the operation worker, not the UI thread.
No history schema or wire changes are needed; schema 8 remains current.

A folder can remain nonempty because it contains versions protected by Recovery.
Keeping it must preserve those records and files, including across reopen and
ordinary history expiry. The existing protected-version catalog and atomic
created-container Keep transaction retain their authority; this decision does
not move or discard any saved versions.

Public tests cover direct folder creation, Skip/Cancel/reopen/Keep, nested copied
containers after ordinary leaves reverse, retry after moving new work out,
stale input/cancellation, and protected Recovery inside a kept container through
expiry and copy-out. Native dark/light acceptance exercises normal folder
creation, the safe default, metadata, previews, retry, stale review and nested
container counts. Exact results are recorded in `created-folder-verification.json`.

This decision covers an owned folder with remaining contents. Changed/replaced
or unavailable created-folder locations still need dedicated choices. Broader
batch dependencies, directory Merge/restoration and interrupted-directory recovery
remain part of the active full product goal.

Final core validation passes 183 tests with thirteen parent-invoked child
helpers. Five new public folder tests pass. Build, all-target Clippy with warnings
denied, formatting, Python syntax, maintained inventory and whitespace checks
pass. Native 57b dark and 58 light at 1.25 scale each pass fourteen checks with
clean GTK logs. Rendered metadata and kept-file preview were inspected. Native
57 stopped in the fixture before its first folder operation because the empty
notification stream had no file yet; the fixture now treats that as an empty
list. Initial nested-copy tests also wrongly cancelled progress callbacks;
correcting them to Continue needed no application change. Diagnostic attempts
remain separate from the final proof, with the same tested desktop binary.
