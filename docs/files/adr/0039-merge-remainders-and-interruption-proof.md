# 0039: Review remaining source work after a directory Merge

Status: accepted under delegated implementation authority; verified for the scope below.

After the recorded children of a directory Merge return, new or deliberately
kept contents may remain in its incoming folder. Undo now shows a separate
remaining-items review with both real folder locations. Keep remaining items
leaves that folder and all its contents in place and finishes its marker. Skip
is the Return-key default; Skip and Cancel retain the decision for later.
Already returned children and newer destination work stay where they are.

Keep checks both physical root identities, both shown directory metadata
versions, ancestor sessions and cancellation before its transaction. A changed
shown directory version requires a fresh decision. A noninteractive
Undo reports that a decision is needed before starting a root-removal intent,
so its error cannot consume the pending marker. This review does not open the
remaining files or remove any of their contents.

The transaction consumes only the finished marker. It updates the captured
parent references needed by older returned children, without adopting the
destination root for older primary ownership. Any saved previous destination
that cannot be restored while the source folder remains becomes protected
Recovery data. It is not recorded as restored. Outcomes count the source folder
as kept, add zero reversed changes for this decision, and separately report any
previous destination not restored. Its Recovery record survives history expiry.

For a Merge from Trash, keeping a remaining folder also keeps its matching
Trash metadata, so that remaining work stays listed and inspectable. Ordinary
completion of an empty source still removes its Trash metadata. No persistent
representation changed; history stays schema 9.

Fresh contents are not silently added to the old Merge snapshot. In particular,
a new source file can reuse the name of a child that has already returned.
Blindly re-expanding that name and applying the old Keep both relocation would
retarget older history or protected versions belonging to the returned child.
Keep remaining items leaves those namespaces intact. Explicit Merge/rescan of
additional items remains planned and needs causal ownership of each generation
and returned namespace; it must not be implemented as a blanket path rewrite.
The full product goal and full directory Merge scope remain active.

Public acceptance now kills a real process after observing the first returned
child's physical identity, with a final collision preventing root completion.
Reopening reconciles it and returns unfinished children without duplicating
completed ones. A separate test proves an unfinished Merge survives ordinary
history expiry and can finish after newer entries are undone. These establish
the tested child-install/restart and retention behavior; process kills at every
root/saved-destination cleanup phase remain required.

Other public tests cover the initial remaining-items failure, Skip/Cancel,
noninteractive retry, stale/cancelled Keep, protected previous destinations
through expiry/copy-out, Trash catalog lifetime, and older history when new
source work reuses a returned child's filename. Native acceptance adds the
remaining-items review, Skip default, Cancel, stale Keep, truthful counts and
previewable kept work. Exact final proof belongs in
`merge-remainder-verification.json`.

Final core passes 207 tests with fourteen parent-invoked child helpers. Seven
new parent tests cover the scope above. All-target Clippy with warnings denied,
desktop build, formatting, Python syntax, maintained inventory and whitespace
checks pass. Native66 dark and67 light at1.25 scale each pass25 checks, totaling
50 on one schema-9 binary with clean GTK logs. Four new dialog/preview captures
were inspected. No native fixture repair was needed. The initial generic
WouldBlock failure remains separate from the final green evidence.
