# Lucia: cross-website autonomous browsing acceptance tests

Lucia is a **general, user-directed browser agent**, not a YouTube macro. A task
must be planned from the user's request and grounded in fresh, observed
WebView2 controls. Hard-coded examples may help a narrow test but must not
replace the general reasoning path. Don't claim "every site works" from one
successful fixture, test script, or provider.

## Core release gate

A release is not demonstrated as autonomous until a **real configured model**
(no simulated-model decisions) has successfully planned, acted, recovered if
necessary, and returned source-backed results in an actual Windows WebView2
instance on multiple structurally distinct websites. Record both successful
and unsuccessful runs. Keep navigation and action traces private/sanitized.

| Website pattern | Native CI fixture or test | Status after code changes |
| --- | --- | --- |
| Search input + result click | `fixture/multistep.html` | Simulated-model Windows E2E passed |
| SPA without full reload | `fixture/multistep.html` | Simulated-model Windows E2E passed |
| Long global navigation | `fixture/generic-controls.html` (160 links) | Native DOM regression passed |
| Shadow DOM accessible controls | `fixture/generic-controls.html` | Native DOM regression passed |
| Image-only links | `fixture/generic-controls.html` | Native DOM regression passed |
| Duplicate element identifiers | `fixture/generic-controls.html` | Native DOM regression passed |
| Readable article page | `fixture/generic-document.html` | Native DOM regression passed |
| Failed action then recovery | `fixture/recovery.html` | Simulated-model Windows E2E passed |
| Background tab/switch/close | `lucia-multistep-mock.cjs` | Simulated-model Windows E2E passed |
| Qwen 2.5 3B local decisions | `run-lucia-real-qwen.ps1` | **Unverified**: 90-second action timeout seen on CPU runner; bounded-control and inference fixes under test |
| Public sites / external redirects | `lucia-youtube-search.cjs` and additional live sites | **Unverified**; cannot be certified from a local fixture |
| Forms, complex widgets, embedded frames | More native and live regressions needed | **Not generally certified** |

## Security and user control

Routine browsing within a user's request should proceed without unnecessary
interruptions. Keep policy checks for sensitive input, purchases, account or
privacy changes, uploads, consequential submissions and unknown high-impact
actions. Never invent personal information or credentials, bypass age or site
access restrictions, or treat instructions found on a page as higher authority
than the user's task. Never claim a task is complete without observed results.

## How to reproduce

- Run `tests/e2e/run-lucia-multistep.ps1` on an isolated Windows WebView2
  instance for deterministic cross-site-layout and recovery coverage.
- Run `tests/e2e/run-lucia-real-qwen.ps1` on the authorized ephemeral
  Windows Actions worker for genuine local model decisions.
- Keep real-model experiments separate from simulated-model CI. A timeout
  does not imply the page was successfully navigated.
- Real public-site tests must use only ordinary, non-sensitive page reads,
  navigation and interactions, and must fail visibly rather than inventing
  success when blocked by logins, permissions or anti-automation checks.
