# Reproduce the original configuration startup/FIFO failure

Status: needs-info

The original 2026-09-16 failure was timing-sensitive during a release build.
On 2026-09-21, the unchanged 11-test configuration suite passed five CPU-load and
five build-load repetitions. An initial build reproduced startup deadline failures
in the import suite, but not the precise configuration/FIFO failure.

Use `scripts/stress-configuration.py` and capture the failing test log together
with its pressure measurements, build command/toolchain, free disk/tmpfs space,
and concurrent workload. Obtain a reproducer that distinguishes process startup
delay from configuration preparation before changing production behavior or any
deadline. The prior child failures produced no diagnostic before termination.

Evidence and current limits: `docs/notify/project-completion-verification.md` and its
linked `configuration-*-load.json` artifacts. No flake fix is claimed.
