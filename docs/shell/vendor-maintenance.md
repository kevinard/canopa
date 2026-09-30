# Shared dependency maintenance

The Canopa workspace owns the patched dependencies and their provenance. See
[the root maintenance guide](../vendor-maintenance.md) and run
`python3 scripts/check-vendor.py` from the repository root. Application-local
forwarding scripts have been removed.

Historical 0.18.0 patch records remain in Git history. Current source, exact
upstream baselines and retained patches live in root `vendor/`.
