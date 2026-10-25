# 0134: Saved-copy descriptors and created target ownership

Status: scoped foundation verification. Continues ADR0133. Cross-filesystem saved
restart publication, cleanup claims and exact task binding remain unfinished.

A forward saved-container copy retains the actual source folder and destination
parent selected before conflict review. Open previous relative to that held
source directory and create the output through the held destination parent.
Reuse ordinary copy implementation and preserve its checkpoint-before-source-open
ordering. Directory/source and destination checks continue during preparation;
filesystem work remains off the renderer thread.

Saved-copy metadata restoration acts on the actual created file or link FD.
Checking its name and subsequently writing through that path leaves a race.
A public OperationEngine regression substitutes a regular payload after the final
name check: the pathname baseline overwrites that foreign file's timestamp before
refusing completion. FD restoration preserves the substitute's bytes and time,
then refuses the changed namespace. Actual raw symlinks retain their original
nanosecond timestamps without following their targets. This applies to both
forward and reverse saved-copy preparation.

Publishing a created work directory whole keeps its original directory identity.
Convert its captured created-payload proof into saved-previous proof, remove
its disposable-payload rights, and carry that target ownership into all retained
catalog owners in the existing atomic cutover transaction. Do not reconstruct
proof from the current published path. Missing created-leaf proof cannot be
promoted; an older migration without target proof must not replace an existing
strong owner with None and thereby fall back to scalar cleanup rights.

A second public regression reproduces a subsequent Move refusing the stale source
proof left by cross-filesystem cutover. Preserving the actual target proof passes
the cross move, later same-filesystem move and all three Undo steps, including the
original replacement. No serialization fields or schema version change here.

On 871 captured inputs, the backend library passes 152 tests/18 ignored while
explicitly filtering the single known unfinished cross restart gate. That gate
remains an unignored test and fails on the same current source with an uncovered
completed filesystem step; this is not full library or restart acceptance.
The complete saved-container integration target passes 24 tests/1 ignored.
Default all-target Clippy with warnings denied and workspace formatting pass.
Metadata races and target-ownership refusal were reproduced twice; failure roots,
raw links and empty work directories are archived with decoded verification.
Protected app/Niri/earlier restart binaries are unchanged.

See [the WIP](../../../.scratch/file-manager/SAVED-CROSS-WIP.md),
[working design](../../../.scratch/file-manager/SAVED-CROSS-DESIGN.md),
[verification](../../../.scratch/file-manager/saved-cross-foundation-verification-01.json),
and [next work](../../../.scratch/file-manager/SAVED-CROSS-NEXT.md).
