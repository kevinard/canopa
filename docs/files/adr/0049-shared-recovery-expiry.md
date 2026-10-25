# 0049: Preserve saved data until its remaining owners can expire

Status: accepted under delegated authority; verified for the scope below.
The full file-manager goal remains active.

An archived Recovery record can share a saved container with an active Undo
entry or another archive. Expiring that record is a change in catalog ownership;
it is not sufficient authority to unlink bytes still needed by the other owner.
Public regressions prepared from real Copy-replace operations reproduce startup
deleting a live Undo backup and a protected archive's shared bytes. Both current
schema 16 and prior schema 15 input fail. Two unprotected archived owners also
leave an Unavailable catalog remnant when one deletes their shared file first.

Automatic expiry now checks eligibility without mutation before consulting the
physical saved-stage owner index. A recovery flag, changed saved version,
unidentified stage or payload, unexpected payload type, or extra stage contents
prevents both unlink and retirement of that record. Checking known contents
before unlink also fixes the reproduced loss of the ordinary previous file when
an additional user file makes removal of the saved folder fail afterward.

When another owner can still reach the same identified container, eligible
unchanged archive metadata can retire while the bytes remain. The last eligible
unprotected owner removes the payload and empty container once. Protected or
edited owners retain their own metadata and data. No active Undo entry is
removed by garbage collection.

The owner lookup streams the stage index and stops at the first match. It does
not allocate every owner for each archive. Parent aliases may name the same
physical container through different paths, so lexical path equality is not
sufficient. A missing owner alias still protects identified shared bytes, and
the available archive record stays available for copy-out until that owner can
reach its container again. Alias reappearance permits redundant metadata expiry
and ordinary older Undo. An arbitrary matching filename does not establish
container ownership or authorize adoption of another physical version.

The history schema remains 16. This guard uses existing stage ownership and
reference resolution. It does not add a disposal journal or complete broader
orphan/cleanup interruption, directory Recovery, unexpected-payload handling,
directory-descriptor authority or explicit protected-data disposal.

Tests observe public Recovery, Copy, Undo and actual private files. Closed-engine
SQL only prepares shared-owner input with consistent derived indexes; it is
never used to assert implementation output. Seven parents cover active/archive
ownership, protected/changed/extra saved work, last-owner expiry, actual and
missing aliases, copy-out and successful older Undo after reappearance. Native
acceptance adds an unprotected shared archive to the existing stopped-app v15
Recovery context fixture, then exercises protected previews and copies before
and after a Keep both return.

Verification passes 289 file-manager core tests with 19 parent-invoked helper
definitions, all-target Clippy with warnings denied, the desktop build, format,
Python syntax, maintained inventory and whitespace checks. Native 101 dark and
102 light at 1.25 scale each pass 14 checks on the same executable and actual
device IDs 35/2050. All 16 captures were inspected; one protected row remains
after shared expiry and return, with the original filename and protected text
in Quick Look. Both private destinations were removed and GTK logs have no
warnings, criticals or panics. The 99 Rust/build source hashes stayed unchanged
through verification. Detailed red/green evidence, source manifests and precise
scope are recorded in `.scratch/file-manager/shared-recovery-verification.json`.
