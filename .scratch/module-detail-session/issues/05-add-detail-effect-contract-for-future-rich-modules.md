Status: ready-for-agent

# Add Detail Effect Contract for Future Rich Modules

## Parent

.scratch/module-detail-session/PRD.md

## What to build

Lock in the effect ownership contract for future rich **Modules** without implementing network or audio detail views. **Module Detail Content** should be able to request app-owned effects for shared app infrastructure, while keeping module-owned effects available for domain-specific runtime work in future network/audio implementations.

This slice should document and test the contract enough that future network and audio detail work can reuse centralized app behavior such as command policy or panel close behavior without reimplementing it inside a **Module**.

## Acceptance criteria

- [x] The detail effect contract clearly distinguishes app-owned effects from module-owned effects.
- [x] App-owned effects can be returned from **Module Detail Content** and handled by app-owned infrastructure.
- [x] The contract does not require a **Module** to reimplement centralized command execution or process launch policy.
- [x] Module-owned effects remain representable for future runtime-adapter work such as network connect/disconnect or audio source switching.
- [x] Tests or documentation cover the intended ownership split without adding full network or audio detail functionality.

## Blocked by

- .scratch/module-detail-session/issues/02-route-opaque-module-detail-messages.md
