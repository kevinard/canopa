# 0020: Apply Iced widget transitions before later queued input

Status: accepted under delegated implementation authority, 2026-10-18.
Queue regressions and scoped Files input verification pass; full integration
verification remains required.

Ordinary Iced windows must apply a form transition before later queued keyboard
input traverses its controls. Files Preferences reproduced a real failure with
Alt+2, Tab and typing: the native shortcut arrived first, but Iced delivered the
whole queued event batch against the old Views tree before applying the Toolbar
message. All thirteen Tabs could use Views, leaving the intended filter unedited.
A longer fixture sleep concealed the issue and is not the product fix.

Use Iced's existing widget-invalidation signal as the boundary. Files' synchronous
form focus group requests widget rebuilding when it publishes a page/control/input
message. Traversal itself retains its synchronous widget operation and does not
request a program update. Other ordinary widgets keep their existing signal policy.
The same boundary covers filtered choices and mouse/keyboard form controls, without
teaching the shared adapter any Files commands, pages or path semantics.

The root iced_winit fork delivers each window's queued events up to the first
outdated interface. It reports statuses only for events actually delivered, retains
the untouched tail, applies program messages and rebuilds the interfaces. It then
replays the older tail before newly generated/native events, using a polling turn
to ensure progress. Do not broadcast deferred keys as ignored, duplicate them,
change their window identity, or wait for unrelated input to wake processing.
Normal traversal without widget invalidation does not add a program rebuild.

Keep this renderer-neutral queue seam in the single root fork. Existing upstream
version, revision, archives and licenses remain unchanged; update provenance and
the reviewable patch together. The application keeps ownership of form state and
stale target guards. No toolkit, dependency version or application-specific queue
is introduced. Layer surfaces retain their existing adapter and ownership.

Three std-only queue regressions cover yielding at a page change, repeated page/
filter transitions with exact event order and single delivery, and ordinary/empty
batches. The first two reproduce the old batch behavior before the fix; the queue
contract now passes. The native fast repro remains the integrated regression.
Final Files, shared-stack, workspace, performance and release gates are still
required; neither queue tests nor traces establish their completion. See the
[diagnosis record](../../.scratch/file-manager/ICED-PREFERENCES-DIAGNOSIS.md).

Files' location editor also needs this boundary. A native `Ctrl+L` followed
immediately by a file URI reproduced a missing first character (`ile://`),
leaving the file unselected and Rename unavailable. The browser now publishes
its configured Location shortcut inside the widget event and rebuilds before
later input. The new field receives focus and full selection synchronously in
layout; an asynchronous focus/select task still lost the character even after
the shortcut boundary was added. Prepare selection once per transition so later
typing retains its caret. Browser fields retain native editing and traversal. Native fields receive the
event first; only an ignored chord can activate Location. This preserves text
Copy even when Location is rebound to Ctrl+C and file Copy is disabled.
The private native regression checks actual Rename and shared Undo across eight
fresh windows after a refused collision. This is presentation/input work and
does not move operation policy into the renderer. The separate intermittent
first-window startup timeout remains under investigation.

Clean candidate91a298fd passes both original operation workflows, sixteen rapid
new-window Rename/shared-Undo cycles across dark/100% and light/125%, and actual
Location/Search text Copy with Location rebound to Ctrl+C. All five application
logs are empty. The 47 Iced library tests, all-target Clippy and formatting pass;
both scoped reviews report zero remaining findings. See the
[Location diagnosis](../../.scratch/file-manager/ICED-LOCATION-DIAGNOSIS.md) and
[scoped verification](../../.scratch/file-manager/operation-startup-verification.json).

On the preceding clean executable `826a8c…`, twelve cold original fast probes
pass without diagnostics. The final candidate `a85b1012…` passes both complete
Preferences workflows and the full 14-workflow Files regression matrix in dark/
100% and light/125% (198 declared checks), plus a twenty-cycle resize stress case.
The later viewport-feedback guard is recorded in Files ADR0062. All 17 final
application logs are empty; eight selected captures were inspected. See
[the immutable source-bound scoped evidence](../../.scratch/file-manager/iced-preferences-verification.json).

This does not close shared Shell/Notify native, strict/full workspace, performance,
accessibility, release or the full file-manager product gates. The original stopped
matrix and diagnosis remain preserved, and full GUI migration continues.
