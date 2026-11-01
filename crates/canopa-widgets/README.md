# Canopa widgets

Toolkit-independent data for widgets in the shell, lockscreen and future surfaces.
No Iced dependency, windows, action dispatch, credential access or global runtime.

- Battery models and UPower signal reader, shared by shell and lockscreen.
- Media models, selection reconciliation and a read-only observer of the shell's
  selected-player interface. Shell still owns player selection and controls.
- Bounded, freshness-checked local weather snapshots. The external writer owns
  location, credentials and network policy.
- Bounded passive script capture/observation, with explicit argv, time/output
  limits, changed-value notification and process-group cleanup. Consumers own
  command configuration, placement, visibility and runtime lifetime.

Consumers decide which fields are appropriate to reveal while locked. A future
widget should add a typed snapshot/adapter here, then render it in each consumer;
shell Module/Panel lifetimes and interactive state remain shell-owned.
