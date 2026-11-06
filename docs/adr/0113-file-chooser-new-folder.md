# 0113: Explicit New folder in the Iced FileChooser

Status: implemented; focused unit, public native and real sandbox Save checks pass,
2026-11-05.

SaveFile and SaveFiles need a way to create their destination folder without
leaving the portal. Add a New folder toolbar action and Ctrl+Shift+N in the
existing Iced Picker. A separate name prompt requires an explicit Create action;
Enter confirms it, while Escape/Cancel returns to the same chooser without
selecting a destination. Ordinary file-open dialogs retain their browsing flow.

Keep directory browsing in Files' renderer-independent Browser. The portal owns
this small explicit creation operation without starting Files' operation engine,
resident host or GIO runtime. Both opening the parent and creating the directory
run through the existing filesystem worker, away from the Iced thread. The prompt
retains an open parent descriptor. Recheck its identity at creation, use mkdirat
for a single validated name and refuse every existing entry, including symlinks.
There is no replacement, recursive creation or automatic suffixing.

Preserve original Unix bytes in the parent path. The typed name is UTF-8; reject
empty, dot/dot-dot, separator/NUL and overlong names. Errors remain in the prompt
for correction. After successful creation, enter the child using the existing
cancellable directory scan and preserve the application's proposed Save filename
or ordered SaveFiles names. File acceptance still creates/truncates no document.

An explicitly created folder is a completed user action and remains if the later
save is cancelled. Closing the portal does not promise rollback of a creation
already issued. Finished pickers ignore late results; revision checks prevent old
results from unlocking or redirecting a newer operation. Parent replacement
before creation fails without mutating either directory. A path change during
creation is reported rather than automatically retried or rolled back.

Focused checks cover byte paths, invalid names, existing entries, parent identity,
prompt cancellation, stale completion and retained Save names. The maintained
public FileChooser fixture adds actual Iced SaveFile/SaveFiles flows, including
the keyboard shortcut and toolbar, invalid/duplicate corrections, Unicode folder
names, prompt-only Escape and ordered destinations that remain unwritten.
No GTK, dependency/fork, live routing or Print-specific workflow change is added.

Current acceptance: four new checks plus existing chooser/app-chooser checks
pass (11 focused). The complete Portal library passes451/20ignored and the
FileChooser wire/lifetime integration passes. Strict workspace Clippy, the normal
workspace binary build and formatting pass on the original feature graph.
Both dark/100% and light/125% pass15 public desktop groups on backend6d08eae2.
The first dark case remains failed because its toolbar click missed the client
title-bar offset; correcting that fixture coordinate needed no Rust change.

The existing real Flatpak probe additionally creates the folder in Iced, accepts
Save, then writes through its standard Documents URI into the new child. Its
original unshared file remains unchanged and the helper loads no GTK. Both
grants/identity groups pass on the same backend with the existing runtime and
companion binaries. No SDK/runtime download or another probe build is needed.
Receipts: target/portal-new-folder-88/ and
target/portal-new-folder-flatpak-88/native-dark-01/summary.json. The latest whole
workspace test gate remains phase86; these focused results do not relabel it as
a fresh full-workspace acceptance run.
