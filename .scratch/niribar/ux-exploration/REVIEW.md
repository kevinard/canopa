# Review of the approved panel implementation

Baseline: the working-tree change against `55bb76c`, including new panel motion files.
Spec: `PRD.md`. Two independent read-only reviewers checked standards and spec.

## Standards

No documented-standard violations were found. One nonblocking duplication
heuristic: the header height was duplicated between sizing and rendering.

Resolved by shared header padding and control-height constants; the preferred
height is derived from those same values.

## Spec

Two correctness findings were raised:

- Animated controls were drawn at a translated position while hit testing used
  their resting position. Resolved by inverse-transforming cursor and positional
  mouse/touch events using the last displayed frame. A widget regression activates
  visible controls outside their resting bounds on all four edges.
- An in-place provider replacement during entry could update live controls behind
  a cached old image. Resolved by ending entry when accepting a provider update,
  preserving its window and focus. An application regression covers this case.

The follow-up spec review accepted the coordinate correction and found the root
capture/reset structurally consistent with Iced's render lifecycle. Native
rendering, capture cost and visual behavior were left to desktop validation.

Standards: one heuristic resolved, no hard violations. Spec: two correctness
findings resolved. No outstanding review findings.

The final follow-up found no unresolved correctness issue in provider settlement
and confirmed that leaving `Preparing` untouched preserves the scale handshake.
