# 0116: Prepare copied leaves through opened sources and stages

Status: implementation and verification in progress, 2026-10-22.
Continues ADR0114. Executable restart continuation remains unfinished.

The shared copy helper took its source metadata before stage registration, then
read a link target by pathname. A deterministic registration callback replaces
the link. The helper reports failure but has already staged the replacement's
target. A second regression replaces the staged file during a real progress
callback. The helper reports success and adopts the replacement's identity with
the original data's digest. Both failures are recorded in restart-leaf red logs.

Pin the source leaf with O_PATH/NOFOLLOW relative to its opened parent before
stage registration. Verify its type without opening devices or FIFOs for I/O.
Read a final link through that actual descriptor, preserving raw Unix bytes.
For a regular file, reopen only the pinned regular object through its process
descriptor link. Namespace checks supply refusal, never a new readable source.
Check both the source snapshot and its parent/name bindings after callbacks,
including the final checkpoint before returning prepared data.

Linux [readlinkat(2)](https://man7.org/linux/man-pages/man2/readlinkat.2.html)
supports an empty path for an O_PATH/NOFOLLOW link descriptor.
[proc_pid_fd(5)](https://man7.org/linux/man-pages/man5/proc_pid_fd.5.html) describes
descriptor links to actual open files; reopening still checks access permissions.
Only an already-typed regular source reaches that reopening path.

Create private work through the actual destination parent. Keep its directory
and new payload open throughout preparation. File writes, permission/time
restoration and final metadata use that payload descriptor. Link creation and
ordinary publication use operations relative to the held directories. Refuse
changed parent, stage or payload bindings before publishing, and preserve a
refused journal through the existing destination-attention path.

Schema31 adds optional parent/directory/payload Original facts to Stage. Record
the empty stage first, then register the actual new leaf before its first byte
write. A lost second registration leaves an unrecorded leaf for review. These
facts are obtained from the actual descriptors. They do not invent proof on
unsupported filesystems or upgrade legacy stages into execution owners.
Where these facts exist, cleanup validates them and uses the opened directory;
unknown payloads and replacement parents/stages/leaves remain intact. It only
removes empty directories and never recursively deletes an unknown tree.

The first Forward-publication refusal regression also found that recovery
cleared its intent before attempting cleanup. A new refusal therefore lost its
Recovery record and made the next engine open fail. Uncommitted work now cleans
before clearing its existing journal. Completed changes with recorded stage
facts append their change and a Cleanup intent in one SQLite transaction.
Cleanup checks the exact committed operation/ordinal/path before retiring the
stage; a failure leaves that record available for Recovery and retry. Schema31
reserves this interpretation from older executables. The Cleanup intent does
not replay the completed filesystem change.

This is a prerequisite for restart execution, not its authorization. Generic
legacy/unsupported stage handling, journalled cleanup claims, destination
replacement claims, source removal/empty-folder claims, same-filesystem Move,
exact step association and typed handoff/abandon lineage remain unfinished.
The required public killed-process Retry/Continue gate must still pass before
Files can claim safe executable continuation after restart. Other operation
families and the remaining full-goal areas remain active.
