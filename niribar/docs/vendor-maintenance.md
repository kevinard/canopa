# Shared dependency maintenance

The Canopa workspace owns the patched dependencies and their provenance. See
[the root maintenance guide](../../docs/vendor-maintenance.md) and run
`python3 ../scripts/check-vendor.py` from this application directory. The old
`python3 scripts/check-vendor.py` command forwards to that checker.

Historical 0.18.0 patch records remain in Git history. Current source, exact
upstream baselines and retained patches live in root `vendor/`.
