# Allow Explicit Shell Commands by Default

Niribar allows explicitly configured shell commands by default for command-backed modules such as script and button because local status bar users expect shell snippets for personal automation. This is local code execution, so command-backed modules must require explicit user configuration and run under a bounded execution policy rather than being synthesized from defaults.
