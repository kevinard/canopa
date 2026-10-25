# Operation startup checkpoint

Current: scoped checkpoint COMPLETE, source21/558 inputs, clean binary91a298fd.
All known compiler/native/archive/cache handles are terminal. Both code reviews20
(16 files) and both final documentation checks21 (5 files) report0, all hashes
verified. No live validation to resume. Entire product goal remains ACTIVE.

Evidence: operation-startup-verification.json; operation-startup-preservation-21.json.
Archive21 1580596B SHA9d4dac12f20389810c137bf576e4e7e67b348c970a927cbbe9b817f200e3180e;
binary119327848B SHA91a298fd33f66cbc94406e48d734060a5b8b27a7a1abac8117978f12e2fe48ad;
gzip29024387B SHA05f8ea7b416cf1e366aa247c790c394ca58edd6642dd5273aece1f8ad7a230c0.
All558 source members and decoded binary verified. Final21 differs from validated20
only in four existing documents and a new Location diagnosis document.

Implemented ADR0081: LocalOperations owns volume monitor/queue before Iced; prepared
Storage handoff gates history/work. Startup failure/EOF visible; final owner Drop
releases waiter and rechecks stop after readiness. Iced attaches/observes. This is
not a daemon or activation interface. ActionJobs startup and attention handoff still
need host ownership; see next notes and ADR0076.

Validated core46013: 76 parent tests+2 isolated helpers, six public queue/storage/
clipboard suites; domain code unchanged thereafter. Original focused21/source06
and genuine late-readiness red28297 retained. Final serial67837 exit0: all-target
Clippy3 existing warning groups, Iced47, fmt. Final native4948 exit0: original
operations dark/light12+12, rapid window-navigation8+8, rebound native editing3;
5 workflows43 checks, all5 application logs empty. Representative captures inspected.

Location diagnosis: burst source12/f2a2efac genuinely lost first URI character;
shortcut boundary alone13/ae5e91b5 still failed; synchronous layout focus/select14
passed8. Spec15 found rebound CtrlC steals native Copy; corrected fixture19/25433
reproduced actual text Copy failure.20 delivers Browser input to native fields
first, activates only ignored Location chords synchronously, prepares field once,
removes async focus/select. Full original/reduced workflows pass clean20. All tagged
diagnostics removed. ICED-LOCATION-DIAGNOSIS.md records setup failures separately.

OPEN: intermittent first-window15s timeout (83274 dark and prior config checkpoint).
Later original workflows and4+16 minimized probes pass, but no cause/fix established.
Slow font initialization7.061s is an observation, not timeout root proof. Keep this
independent from completed scoped input/startup ownership work. Old /tmp finalizer
is obsolete and must not be executed.
