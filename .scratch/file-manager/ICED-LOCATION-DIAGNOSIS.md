# Rapid Location entry and native editing

The original light operation acceptance (83274/source06) reached a new Browser
but failed to open Rename. The capture showed no selected file and an ENOENT
footer. Eight slow-input cycles passed with a wide parent and eight with a narrow
parent. Sending Ctrl+L and a file URI in one native wtype burst reproduced the
failure on source12/f2a2efac: the submitted target began `ile://`, missing `f`.
Evidence: operation-window-repro-03 (terminal1, command exec-f90774c5-cd3a-4c6a-8354-3b108016414d).

The application event subscription ran after queued input traversed the old
widgets. Publishing the Location shortcut inside the widget and invalidating
alone was insufficient: source13/ae5e91b5, native25649, still lost the first
character in cycle2. The asynchronous focus/select task could still run after
queued text. Source14/ec2fe029 prepares focus and full selection synchronously
in layout, once per transition; native19113 passes all8 cycles. Later typing
keeps its caret. No shared vendor change was required.

Spec review then found that pre-field interception could steal native text Copy
when Location was rebound to Ctrl+C and file Copy disabled. The first two fixture
attempts had setup defects (unknown command id and a seed clipboard publication
race); they are not product reds. Corrected source19/native25433 reproduces a
real failure to publish the location text, using diagnostic14/ec2fe029.
Source20 delivers Browser input to native fields first and activates only an
ignored configured chord, still synchronously in the widget event. Form page
accelerators retain their original precedence. A private native test checks
actual clipboard markers from Location and Search, then rapid rebound URI
navigation with actual Rename and shared Undo.

Clean source20, binary91a298fd, native4948 passes all5 workflows/43 checks:
original operation workflows both themes, 8 rapid cycles per theme and rebound
editing light/125%. All5 application logs are empty; representative narrow Undo,
shared-window and selected-file captures were inspected. Core76+2, Iced47,
Clippy, formatting and both16-file reviews pass. All tagged temporary diagnostics
were removed before this clean candidate. See operation-startup-verification.json.

The separate intermittent first-window timeout remains unresolved. Four then
sixteen minimized startup launches passed; the traced first font initialization
once took7.061s. That is not proof of the timeout's cause. Do not describe it as
fixed or use those capture-inclusive timings as a pure window-latency benchmark.
