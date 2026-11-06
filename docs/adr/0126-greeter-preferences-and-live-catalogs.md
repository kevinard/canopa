# 0126: Greeter preferences, live catalogs and power actions

Status: accepted

The greeter persists only successful user/session-ID choices, after greetd
acknowledges session start. Configured default-user overrides the remembered user.
`canopa-file` supplies descriptor-checked private reads and atomic durable writes;
the greeter owns schema, budgets and the success-only save boundary. Preview and
configuration checking remain free of preference I/O.

AccountsService signals and local inotify events refresh account/session/avatar
catalogs. A reusable bounded `canopa-file` watcher observes parent directories for
atomic replacement and missing-source creation; overflow rebuilds watches. No
periodic filesystem scan is introduced. Remote NSS sources can be explicitly
refreshed with F5. Avatar invalidation cancels stale generations and clears the
bounded thumbnail cache. Typed input survives unrelated catalog changes.

The authentication attempt retains its selected session command. A changed/removed
desktop cancels an in-flight attempt; discovery cannot redirect authenticated
handoff. Once handoff has started it remains greetd-owned.

The greeter displays shutdown/restart only for logind's `yes` policy result. User
confirmation is a separate step, followed by a fresh policy query and logind's
authorization on the actual request. The unauthenticated greeter supplies no
administrator identity for a `challenge` result. Power actions do not persist
login choices. See [setup](../greeter.md#remembered-choices-refresh-and-power).
