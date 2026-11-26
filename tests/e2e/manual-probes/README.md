# Manual Lucia native probes (25 November 2026)

These nine source-only probes were recovered from the temporary `Browsphere-google-fix` worktree before cleanup. They are **not** part of automated CI, and have not been re-run after migration. All `.cjs` and `.py` files passed syntax checks.

- `lucia-google-live-smoke.local.cjs`: Google navigation and Lucia task smoke probe.
- `lucia-objective-python.local.cjs`: Python documentation objective / source verification probe.
- `lucia-play-any-song-*.local.cjs`: variations on the browser music-task acceptance probe.
- `inspect-music-player.local.cjs`: page media-state inspection via a local Chrome DevTools Protocol endpoint.
- `inspect_*music*.local.py`: isolated SQLite agent-trace inspection helpers.

Use only in a disposable local test profile. Some scripts attach to a localhost CDP debug port and may change local **test** model settings or print agent traces; never expose the debugging port publicly or run on sensitive user profiles. Verify assumptions and executable version before use. Their presence is not evidence that these workflows passed.
