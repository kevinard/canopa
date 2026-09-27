# Extract the shared Canopa panel protocol crate

Status: implemented
Type: AFK

## Parent

[Canopa Notify PRD](../PRD.md)

## What to build

Extract Niribar's external panel protocol types into a small shared Canopa protocol crate that both Niribar and Canopa Notify can depend on. The shared crate should preserve the existing provider protocol behavior and avoid making Canopa Notify depend on the Niribar application crate.

## Acceptance criteria

- [ ] The shared protocol crate contains the external panel protocol types needed for provider handshake, content replacement, and user interaction events.
- [ ] Niribar can use the shared crate without changing the intended external panel provider behavior.
- [ ] Canopa Notify can depend on the shared crate without depending on the Niribar application crate.
- [ ] Protocol versioning remains explicit.
- [ ] Existing Niribar protocol tests continue to pass or equivalent shared-crate tests cover the moved behavior.
- [ ] Documentation notes the dependency direction and how Canopa Notify should use the crate.

## Blocked by

None - can start immediately
