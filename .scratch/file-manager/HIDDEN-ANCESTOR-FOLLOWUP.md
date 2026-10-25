# Hidden-ancestor selection follow-up

Status: resolved for the recorded scope under
[ADR 0034](../../docs/files/adr/0034-hidden-folder-navigation-context.md).

Rendered inspection of native cross-filesystem Undo exposed an ancestor column
highlighting `config` while the browser was actually inside the hidden private
`.canopa-native-cross-...` folder. The active hidden child was omitted by the
ordinary hidden-file filter. Public Browser reproduction also showed that going
to the parent lost the hidden child just left.

The Directory projection now includes its actual navigation child without
revealing hidden siblings. Browser parent selection retains the child through
that same rule. Refresh/history preserve the temporary context; an explicit Hide
hidden clears it. The global Show hidden setting stays off. A raw Unix-name
hidden directory-link regression also passes.

Fourteen focused Browser/search/preferences tests, build, Clippy and native 55b
(dark) and 56 (light at 1.25 scale) pass. The 24 native checks verify wide/narrow
Left/Right, hidden sibling exclusion and Quick Look selection continuity.
Rendered inspection confirms the correct ancestor and explicit-hide results.
Exact scope, hashes, logs, screenshots and the fixture tooltip correction are in
`hidden-navigation-verification.json`. The full product goal remains active.
