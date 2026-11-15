# Canopa launcher contracts

Launcher preference and discovery types shared by Canopa Shell and Canopa Settings.
The Shell owns searching, actions, extension processes and the panel; Settings
edits the preferences through the same bounded, private, locked file store.

- Preferences (`$XDG_CONFIG_HOME/canopa/launcher.json`): bundled extension
  switches and keywords, the web search template, search shortcuts, and pins and
  aliases, which keep their original target and Unix path bytes.
- Extension discovery (`$XDG_STATE_HOME/canopa/shell/launcher-extensions.json`): a
  read-only catalog of capabilities and status messages that the Shell publishes,
  never containing queries.
- Clipboard options and saved snippets.

See the [launcher guide](../../docs/launcher.md).
