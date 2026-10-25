# Ancestor column heading diagnosis

Status: closed, false visual finding. No application change needed. The full product goal remains active.

The prior claim was a mistaken reading of native-directory-move-78 preview captures03/04/06/08. Quick Look covers the ancestor heading centers. Those pixels cannot establish that old labels remain. The visible current-folder label is correct. The earlier open-finding text is superseded by this correction and column-heading-verification.json.

A private native feedback loop cold-launches NewOcean/NewIsland/NewHarbor, navigates to OldPlanet/OldForest/OldTrail, returns, opens Quick Look, then closes it. Every stage has a full capture. The heading strip after navigation and after preview closure exactly matches its fresh-window reference. Direct visual inspection confirms the three literal Old* and New* labels; OCR of all four visible heading strips in run80 independently captures those names (NewIsland has the expected OCR I/l ambiguity). No stale label is visible.

Runs80 and82 dark and81 light1.25 all pass. Sessions24743 and53407 are terminal0. The source is the explicitly diagnostic harness diagnose-column-headings.py under .scratch; no production instrumentation or rendering workaround was added. Current desktop binary remains3efaeb87625ae18dc1260c5fa4e64359e389ee229342c1443b36809cb55f29b3. This verifies the exercised wide navigation and preview-close flow, not the full product.

The diagnose workflow stopped before hypothesis/fix phases because the supposed bug was not reproduced and the original artifact interpretation was disproved. Accessibility instrumentation was unnecessary once literal rendered labels and direct pixel equality provided the evidence. Future heading QA should expose the entire header with the preview closed and compare expected names before filing a rendering finding.
