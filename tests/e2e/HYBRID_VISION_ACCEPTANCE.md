# Hybrid browser, MCP and vision acceptance

## Implemented contracts

- An explicit `mcp workflow` can interleave public HTTPS browser observations with approved read-only MCP calls. An observation can include an optional, user-specified exact public excerpt. The native observer must verify the URL, title and excerpt. A later explicit call may reference only `/structuredContent/excerpt` with a declared dependency. No arbitrary page text, credentials, images or model-generated URLs are forwarded.
- `auto` nodes support optional `joint_selection: true`. Up to three explicitly named providers are discovered; a bounded, round-robin catalog of at most eight approved read-only tools is offered to local inference. Provider-qualified identities preserve bindings even for duplicate names across providers. At most one chosen tool is dispatched per selection. A post-dispatch error or failed expected-value check is terminal.
- Vision may retry an actually changed control layout after failure or stall, within the original three-call budget and cooldown. Fresh native bounds, identity, relevance and hit-testing are mandatory.
- Same-document, same-viewport screenshots can be compared after a coordinate action. Only the encoded-frame change signal is emitted. The image and digest are not stored, and neither pixel/encoding difference nor similarity counts as a verified action effect.
- The independent test oracle validates terminal state, verification, approval status, and tool dispatch order/receipts for the graph cases.

## Repeated native browser actions

An explicit workflow may now include multiple `follow_link` nodes anywhere in its dependency graph (within the 24-node explicit plan budget), rather than requiring a terminal browser handoff. A node includes `source_url`, `label` (literal string or explicit dependency reference), `url`, `title` and an optional exact `excerpt`. Source and destination must be distinct same-origin public HTTPS URLs. The adapter binds a unique visible anchor, checks the navigation and click policies, and revalidates its ID, source, href, label, visibility and hit-test immediately before the native click. It checks the exact destination, title and optional excerpt before completing the graph node. Subsequent MCP steps can use the verified excerpt, not arbitrary page content. A missing, blocked or mismatched action stops dependent dispatch. New-tab, download, form and consequential actions are excluded.

The `mcp-browser-alternating` acceptance case requires a five-node MCP -> native link -> MCP -> native link -> MCP sequence, exactly one invocation per node, and two separately verified browser dispatch receipts. It uses a read-only local MCP fixture and public Rust documentation in native WebView2; it is not a test of arbitrary model-driven browsing.

## Resumable and conditional browser specialists

The `browser_task` node delegates a read-only natural-language objective to the browser runner and returns to the parent scheduler only after the specialist and exact native URL/title/optional excerpt checks pass. The explicit plan limit is now 24 nodes; the underlying graph is capped at 32. Child graphs have separate bounded execution states.

A browser task may declare up to three distinct `dynamic_tools` rules. A rule contains `trigger_url`, `trigger_excerpt`, approved `servers`, `task`, and an `expect` check. A matching fresh public observation inserts a tool prerequisite in the active child graph, suspending the stage until the tool result matches. The browser then reobserves the page before proceeding. Missing required triggers, invalid results or unavailable authorized tools fail closed. This is conditional, user-authorized dynamic insertion, not arbitrary tool invocation from page instructions. The model selects only from approved read-only tool catalogs.

The scope includes the initial origin and exact declared destination origin; entry across origins requires the exact declared URL and the existing navigation approval flow. Forms, writes, purchases, downloads and arbitrary new origins remain unsupported in intermediate specialists.

`mcp-browser-dynamic` requires a real native browser child, a dynamically inserted MCP tool receipt, resumed browser verification, a later parent tool invocation and a single final verification report.

## Run

```powershell
cargo test -p browsphere-core --offline --locked -- --test-threads=2
cargo build -p browsphere-core --offline --locked
python tests/e2e/run-lucia-phase3.py --self-test
python tests/e2e/run-lucia-phase3.py --case mcp-browser-alternating --case hybrid-vision-interleaved --case mcp-joint-selection
python tests/e2e/run-lucia-phase3.py --focus hybrid-vision --approved-exa --repeat 2
```

The interleaved benchmark uses a local read-only MCP fixture, public Example Domain and IANA pages, real WebView2, and the configured local visual model. The joint-selection case uses local MCP adapters backed by public GitHub/PyPI APIs, requires both catalogs to be discovered before dispatch, and independently checks the dispatch order and final native destination. Level 4 uses the already-approved authenticated Exa tool only when explicitly enabled; a missing retailer price cannot pass.

## Remaining limitations

Repeated explicit observations, links and bounded read-only natural-language browser tasks can return control to the DAG. Model-created arbitrary tool objectives, unrestricted cross-origin clicks, form input and unbounded graph execution remain unsupported. Conditional dynamic tool rules are declared in the approved plan; their insertion is decided by live observations. Frame comparison is not semantic image diffing. General chart/table/image record extraction, broader dynamic-site robustness, production installer validation and repeated held-out mixed-workflow tests remain release gates. A compiled or simulated test is not evidence that the native live benchmark passed.


## Adaptive lookup proposal

A browser task may include one `adaptive_lookup` object with explicitly approved `servers`, a read-only `objective`, an exact `public_context` excerpt and an `expect` scalar check. On a fresh public page containing that authorized context, the local model selects whether a lookup is needed, the operation (lookup, verify or compare), and an exact contiguous span of the context. Native validation constructs a bounded tool objective; arbitrary generated instructions, invented evidence, additional providers and new result checks are rejected. The observation is repeated before dispatch. A required adaptive lookup must produce a verified result before the child finishes. At most three distinct observed pages may be declined; no uncertain dispatch is replayed. The test `mcp-browser-adaptive` exercises model proposal, joint tool selection, insertion, native browser verification and later parent MCP transfer. This is scoped model-generated lookup intent, not unrestricted tool autonomy.


An explicit optional `entry_url` permits an intermediate browser specialist to start from a public page on its declared destination origin. The baseline navigation approval must succeed before a cross-origin entry is dispatched. Exact fresh entry URL and public observation are required before the specialist runs. The isolated `mcp-browser-entry-approved` and `mcp-browser-entry-rejected` cases simulate one exact approved or rejected public navigation decision, check the native destination or unchanged source, and check downstream tool dispatch or dependency blocking respectively. No unexpected or non-navigation approval is accepted by the harness.

A second adaptive lookup mode accepts an exact explicitly authorized `source_url` instead of `public_context`. The local model receives up to twelve visible same-origin public HTTPS link candidates and selects a lookup operation and exact observed link URL. Rust checks the candidate again on a fresh native observation before tool dispatch. Link labels remain local inference context, not external tool arguments; account paths, query-bearing URLs, hidden controls and other origins are excluded. The `mcp-browser-link-adaptive` acceptance case expects a model-selected Python Tutorial URL before native browser navigation and a separate parent MCP excerpt lookup. This does not allow arbitrary model-created external destinations or bypass provider approvals.


## Native structured data extraction

Use one of the explicit requests `Extract tables from this page`, `Extract charts from this page`, or `Extract tables and charts from this page` on the active public HTTPS page. The native host observes bounded rendered rectangular HTML tables with unique column headers, and labelled SVG chart mark groups with explicit visible category/value text. It captures twice, checks source and title, validates shape and numerical values, and reports exact transcriptions with the source URL. No local or cloud model invents values. No browser writes or MCP calls are made. The feature is disabled in private browsing and does not accept arbitrary account pages, hidden fields, merged cells, nested tables, unlabeled marks, canvas/raster charts, or inferred series/axis values. Larger or unsupported regions are counted as omitted; no claim of complete page extraction is made. `python tests/e2e/test_structured_data.py` exercises the production extraction script in isolated Edge fixtures; `structured-table` tests native Lucia on a public HTML example.

The `structured-chart-fixture` case uses an isolated native WebView2 synthetic chart DOM with known labels and explicit values; it does not represent an independently hosted public chart or real-world raster accuracy.

Explicit conditional tool tasks of the exact form `Use <tool> with <argument> exactly <JSON scalar>` are now bound deterministically only when the authorized catalog has exactly one matching tool and the supplied scalar satisfies its live input schema. Missing, ambiguous or invalid binding fails before dispatch. Other lookup objectives continue to use model-based tool selection. This repairs intermittent pre-dispatch selection failure without retrying a dispatched operation.

## Pixel chart reading (provisional)

On explicit chart extraction requests, the native host may capture up to two fully visible, captioned canvas/image regions using bounded lossless WebView2 screenshot clips. Only literal loopback Ollama vision inference is allowed. Two structured readings of printed category/value pairs must agree, and the region geometry and exact screenshot must remain stable. Unsupported, ambiguous and changing regions are omitted. No axis-based estimates or pixel-only values are represented as verified findings; the model consensus is shown as partial evidence, not independent verification. The separate `pixel-canvas-fixture` and `pixel-image-fixture` cases use known synthetic charts in isolated native WebView2, and require exact printed values plus an unverified terminal state. Public heterogeneous chart accuracy is not established.
