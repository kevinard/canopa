# Iced editing state and action icons checkpoint

Current goal turn: progress. The entire PRD remains active and incomplete.
No external blocker, live compiler, private desktop session or delegated agent
remains. Do not mark the goal complete or replace it with this scoped work.

## Implemented and verified

`toolbar_layout.rs` keeps the native location input at child 0 through responsive
layout. Wide mode reserves its configured command position; compact mode places
commands below it. `action_editor.rs` keeps the notice/form sibling positions
stable. A real native red on the preceding executable lost location selection
and focus, and later arrow keys navigated directories. Final native assertions
replace a partial selection after narrowing, insert at an intermediate caret
after widening and navigate to the exact resulting real directory. External
source notices retain a selected draft suffix; a stale save preserves source
bytes/comment and unsaved text. Explicit keyboard discard still closes safely.

`icons_ui.rs` uses shared IconLookup/RasterBatch and application-owned resvg
preparation, one admitted worker, one replaceable pending catalog and guarded
deliveries. Catalog/appearance reload rebuilds prepared handles. SVG resource
references and system-font discovery are not admitted. Symbolic icons use the
shared base foreground; ordinary icons retain authored colors. Missing or
unsupported icons keep their label and executable command. Views perform no
lookup/read/decode. See ADR0061 for exact logical budgets and exclusions.

Current source manifest: `iced-interface-current-source.json`, 475 inputs.
Current executable: 100,995,832 bytes,
`581524070e2666d51a50e7234578ef906335cc797f876f823ce0284b7259baf4`.
`iced-interface-final-candidate.json` and `iced-interface-verification.json`
record source/executable correspondence and actual scope.

33 adapter tests pass in `iced-interface-tests-02.log`. The initial icon test
compilation failed on struct-update syntax for a type implementing Drop; its
explicit initializer fixes that test compile error. All-target Clippy lints pass
in `iced-interface-clippy-01.log`; four existing dead_code groups remain.
Build01, final workspace fmt, Python syntax and tracked whitespace pass.
The normal dependency tree contains Iced/canopa-ui and no GTK widgets.

Final native runs, all on the executable above:

- `iced-interface-final-01/02`: 9 checks each, dark at 100% and light at 125% scaling.
- `iced-interface-actions-01/02`: 15 checks each.
- `iced-interface-browsing-01/02`: 13 checks each.
- `iced-interface-search-01/02`: 16 checks each.
- `iced-interface-previews-01/02`: 15 checks each.

Total 136 declared checks. All application logs empty; 14 selected final captures
were inspected and are listed in the verification record. This is not the full
visual/accessibility matrix or full product parity. The action fixture's own
icon TODO predates and does not establish the separate interface fixture's icon
checks; full icon/condition/focus acceptance remains broader than this fixture.
The earlier focus-only after01/02 runs each pass 5 checks on intermediate executable
baaa24c6a17b2024b7109af1707b85262a782997c610c63f268364f167e2c0ec.
Retain before01 as actual red evidence. Prior proof applies only to its recorded
source/executable, not current code.

## Next work and safeguards

Continue the full Iced port: list/icon views; preferences, sidebar/bookmarks,
columns/details/shortcuts; real filesystem jobs and conflict/Undo/Trash/Recovery
interfaces; devices, file clipboard/DnD/FileManager1; media and syntax previews.
Finish action conditions/configuration/focus/accessibility/source-volume authority.
Connect durable job hydration/source leases/exact filesystem-step continuation
from ADR 0056; never replay arbitrary external commands on restart. Retain every
full PRD/release/review gate in IMPLEMENTATION.md. Do not suppress unused helpers
to claim warning-free workspace checks. No shared fork changes were made here.

Use offline,-j1 Cargo and normal incremental. No Rust edits during compilation;
no compiler/native overlap; private native fixtures run sequentially. Keep the
original user Niri 1238 and PipeWire 1537/1562. `.git` remains read-only; no commit
was made. Preserve unrelated Shell research. Generated obsolete incremental
cleanup receipt: `iced-icons-cache-cleanup.json`; no needed binaries/libraries,
source, evidence, symlink targets or current incremental data were removed.
