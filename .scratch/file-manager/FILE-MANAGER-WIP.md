# Standard file-manager presentation and resident activation

The full PRD goal remains active. This slice adds FileManager1 to the existing
window-independent host; it does not redefine completion around this interface.
Root ADR0025 and Files ADR0099 record ownership and product behavior.

The core owns bounded local-URI validation, grouped presentation, admission and
metadata preparation. Iced owns the resulting windows and display state. CLI
wire protocol1 remains unchanged. The bus worker starts before Iced, never takes
another manager's name, and stops before application queues retire. Optional
activation metadata starts `--daemon` in the session environment. Headless
startup remains viable; a display-free owner refuses windows without losing its
backend. GUI and backend share a process, so crash isolation remains open.

## Scoped verification

- Source07: 696 inputs; default build08 succeeds. Protected executable 133,374,400B,
  SHA256 `5e8e98877de3a394c4dab2ceda83a1b014575060aedc7130f97012c477dc4422`.
  Preservation09 verifies its gzip, decoded bytes and all source hashes.
- Core04: 20 parent tests pass across activation, browsing, presentation,
  metadata and actual private-bus dispatch. The ignored bus helper runs in its
  owned private subprocess and is not counted twice. Core12 reruns that target
  with the added headless-refusal assertions and passes.
- Clippy10: all default targets pass with warnings denied. Lib11: 78 passed,
  0 failed, 1 ignored helper, including checked Properties date conversion.
- Native13 fails because the new fixture root did not match the private GIO
  provider's required `canopa-files-native-` prefix. No provider guard was relaxed.
- Source14 fixes only that prefix. Native15 reaches all workflow checks, but
  screenshot review finds the narrow capture preceding resize and the scroll
  comparison passing on resizing. It is superseded as narrow/scroll evidence.
- Source16 changes only the fixture: wait for committed 340px size and stable
  rendered frames; require a real content change after one End input.
  Native17 passes 13 checks per appearance, dark 100% and light 125%. Inspected
  screenshots in both appearances show raw/hidden selection at 340px and the
  final Properties item after End.
- Regression19 retains six passing individual reports: transport probe 5,
  operations 12 per appearance, headless 4, resident activation 6 and dark
  previews 17. Its process was terminated with status 143 during the last light
  preview. All nine exact private orphaned processes were verified and retired
  through pidfds; only their private temporary runtime was removed. The manual
  receipt labels the batch interrupted. It is not a complete batch pass.
- Regression20 passes the unfinished light preview's 17 checks separately.
  Combined regressions are 68 Files checks and 5 shared-transport probe checks.
  The resident review screenshot was inspected. The actual debug fixture records
  first opening 2.06 seconds and warm openings 1.05–1.62 seconds; these are not
  cold-cache/release benchmarks or a production performance acceptance result.
- Static21: focused formatting, Python syntax, activation-file parsing, package
  shell syntax and all 178 manifest scenarios validate. Primary standards/spec
  review has no unresolved findings in this slice. No delegated review is claimed.

The final source/scope/diff/preservation records and
[verification](file-manager-verification.json) retain exact identities and limits.

The new native fixture uses an actual private session bus and the unchanged
shipped activation file. Its launcher records ownership then execs the selected
Files binary. Cleanup uses exact pidfds and private executable/environment
identity. All original payloads remain unchanged. No host services, defaults,
mounts or user files are changed. The selected Niri companion's exact identity
is recorded and checked again at completion.

Remaining gates include complete archive assembly, activation-token/focus
integration, production idle/warm performance, accessibility, original selected
source and Retry/Continue/restart execution authority, complete native/shared
workspace/optimized/hosted release validation and GTK-reference retirement.
