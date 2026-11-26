# Maintenance scripts

`scripts/maintenance/` contains one-off Python source migration and inspection utilities. These are **not** standard build commands, and some are intentionally tied to a particular prior source layout; review their preconditions before running them.

Windows installer/build entry points remain at the repository root to preserve documented commands and `$PSScriptRoot`-based file paths.
