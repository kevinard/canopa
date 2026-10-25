# Resident host checkpoint — scoped verified

2026-10-19. Goal active; full PRD/session/performance/release gates open. All
known compiler/native/archive/cache handles TERMINAL. No process pending.
This file and NEXT-EXECUTION-NOTES.md supersede earlier in-progress notes.
Exact attempts, logs, limitations: resident-verification.json. Product decision:
root ADR0022 and Files ADR0076. No whole PRD completion claimed.

Application composes configuration, operations/live volumes, actions and
notifications before Iced. local-runtime builds without GUI dependencies.
--daemon starts initially without GUI; normal launches activate the same owner;
--quit drains without answering/cancelling work. Graphical environment needed
at owner start for later Browser windows. No-display owner stays headless,
refuses Browser and accepts Quit. No process crash isolation/service install.
State ownership precedes workers/history. Bounded Unix IPC preserves raw bytes,
config selection and accepted-opening holds; idle exit is atomic with admission.
Independent wakeup survives endpoint replacement; connect+I/O share deadline.
Normal exit awaits history, final volume updates and exact helper reaping.

## Validation timeline (all terminal)

90851/101 interface red;38483/101 sandbox socket refusal;19574/0 approved same
test;56997/0 initial three parents.21384/101 missing exhaustive ActivationPoll
arm fixed;6466/0 initial checks.22669/1 CLI red old88a116cd lacks --daemon.
Review01 Spec found two P2s: pathname-dependent teardown and blocking connect.
66285/101 proves endpoint hang; its backlog fixture invalid (setup cap).
61757/101 endpoint green, backlog setup4096 assertion still invalid. Explicit
listen(backlog1) plus temporarily restored blocking connect produced genuine
isolated red44325/101 (exact child killed/reaped after6s). Restored NONBLOCK
connect;46617/0 five parents+two child fixtures pass.

77897/0 first productionf0b91b8a.80129/1 native C-provider prefix guard rejected
fixture, no shutdown evidence. Corrected fixture then22448/1 real paused-helper
red: owner exited too early. Monitor shared completion await added to
LocalOperations/Application; Drop remains nonblocking. Fixture pins pidfd before
identity checks, waits StateT, always resumes exact child.34685/0 final binary
8acb44cc. Code review02 both axes0/all12 hashes checked before/after.

50139/1 overall: headless4 pass, graphical Niri IPC timeout before Files launch;
unchanged retry31914/0 graphical6 pass (not Files reliability fix). Screenshot
inspected.37553/0 serial47 unfiltered Iced tests/Clippy3existing/no-GUI library/fmt.
22072/0 operations dark12/light12 and actions light15.26389/0 final six activation
parents+three child fixtures, including killed idle-owner stale endpoint reclaim.
87665/0 final test Clippy/fmt/Python/matrix/no-Iced normal tree336 entries.

Native total49/five successful workflows, same PID/raw-byte Rename+Undo after
zero windows/unanswered review+queued work survive Quit/explicit Keep item resumes
and preserves later content/orderly shutdown. Files logs empty except four
expected missing remote GIO monitor messages in actions. Final delta04 nine paths:
Standards0/Spec0, hashes unchanged. Missing receipt created; Standards checked41
evidence paths/exact metrics; Spec checked25 paths and preservation facts.

Private debug first5.366584653s; warm1.207070910/1.021308816/1.118711166s.
Three-second owner+helper initial RSS61424–62556 KiB/~1% core; after GUI closed
194820 KiB/zero CPU ticks. Shared pages/private provider/startup settling apply;
no production/steady-state/cold-cache performance conclusion.

## Preservation

Native source03/578 archive SHA256
058eb46ab19267065412aeb0ee59fad375a1731c19253c5152fa9e2202e22ba0.
Final source04/580 archive1,631,085 bytes SHA256
648a30ac0bc5cb65b4049e3fbd746488c6e70f3d6083d95dbfd6695cdf96412c.
Seven captured paths changed, two existing guides first captured (README/docs
files.md); test/docs only. Do not claim full guide delta as file creation.
All580 current digests and archive members checked after completion.

Binary120,691,824 bytes SHA256
8acb44cc7b7484dcc95ac0d7d3479bde0c6b2db20adbfd2d31b7aea812f7c821.
Gzip29,257,273 bytes SHA256
ee8a92ea5bc55dde276505c7d66ba994eb383c2643257f06cab09911f47fd566;
decoded hash equals binary. See resident-preservation-04.json. Prior checkpoints
retained. Cache cleanup/old app-alias relocation records01/02 retained; all
sequential after terminal under Cargo flock+lockf, original app paths symlink to
verified /tmp copies, gzip decodes checked. Current app/helper/source intact.
Home~153 MiB, /tmp~1.4 GiB free. .git read-only; no commit/PR. Unrelated Shell
audit preserved. Remaining work and rules in NEXT-EXECUTION-NOTES.md.
