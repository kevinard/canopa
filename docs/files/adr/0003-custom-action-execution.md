# Custom action command execution

Status: accepted under delegated implementation authority, 2026-10-16.

Custom actions parse a quoted command into an executable and argument vector.
Selected paths are expanded as arguments, never interpreted as shell code.
Actions that deliberately need a shell can explicitly invoke one. The editor
must show these argument placeholders and reject malformed commands:

- `%f` is the first selected path, `%F` is all selected paths.
- `%n` is the first selected basename, `%N` is all selected basenames.
- `%d` is the current directory and `%%` is a literal percent sign.

The multi-value placeholders must occupy a whole argument. Single-value
placeholders can appear inside an argument. The executable is literal. Paths
retain original Unix bytes, including whitespace and non-UTF-8 names.

Applicability checks the complete selection: allowed files/directories, minimum
and optional maximum count, and case-insensitive filename patterns with `*` and
`?`. Conditions are checked again against current metadata before execution.
Actions with zero required items can run for the current directory.

Actions run off the UI thread in an owned process group. Capture at most 64 KiB
per output stream while continuing to drain output, so a verbose command cannot
block on a full pipe or exhaust application memory. Cancellation terminates the
owned process group, with a short grace period before forced termination.
Scripts retain ownership of their effects; the file manager does not fabricate
Undo entries for arbitrary external commands.
