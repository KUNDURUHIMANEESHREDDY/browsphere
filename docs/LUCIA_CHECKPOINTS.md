# Durable workflow checkpoints

Non-private graph transitions are journaled in a local SQLite WAL with FULL synchronous commits, a 24-hour retention window and a 100-run limit. Graph serialization excludes objectives, criteria, action values, raw page content and tool results. Narrow verified read-only navigation/title destinations may retain a hash of the original goal and observed URL/title. Clearing browsing history clears this journal.

To revalidate or resume an eligible checkpoint, open the previously observed destination and run \`resume checkpoint <run ID> <original task>\`. The run ID is shown with the workflow graph. A fresh native observation must match the original task and destination; the native verifier completes the remaining verification node. No browser actions, submissions, or tool calls are replayed. Private sessions and uncertain or consequential actions are not resumable. Changed pages, expired checkpoints and failed prerequisites fail closed. Completed read-only checkpoints may be revalidated.

Graph amendments are atomic, bounded, cycle-checked and preserve previous dependencies and receipts. The live loop inserts a fresh observation node followed by a recovery node on stalled read-only workflows. This does not implement arbitrary model-authorized new tools, arbitrary cross-session continuation, or unbounded replanning.
