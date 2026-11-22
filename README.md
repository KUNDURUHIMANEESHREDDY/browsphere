# Browsphere

**The browser that works for you.**

Browsphere is an AI-native browser concept built around a simple shift: instead of manually navigating the web one step at a time, users describe an outcome and delegate supported multi-step web tasks to an AI agent.

Browsphere aims to combine a chat-first browser experience with browser automation, task planning, evidence-backed results, and user-controlled autonomy.

> **Turn the browser from a place where users do the work into a place where users can delegate the work.**

## The problem

Browsers are excellent at displaying websites, but people still do the work of searching, opening pages, comparing information, moving between tabs, filling forms, and checking whether a task was completed. General-purpose chat assistants can explain how to do these things, but an explanation is not the same as carrying out a browser workflow.

Browsphere explores a browser that can understand a requested outcome, plan the steps, interact with supported websites, collect evidence, and report what it actually accomplished.

## Product experience

The primary interface is a conversation. A user assigns a task in natural language; Browsphere plans and executes it through the browser.

An optional transparency view lets users inspect meaningful execution activity. Users can hide this view when they prefer a simpler chat experience. Hiding the activity view must not disable the agent.

### Intended task lifecycle

1. **Ask** — The user describes the outcome they want.
2. **Plan** — Browsphere turns the request into clear, actionable steps.
3. **Execute** — The browser agent navigates supported websites and performs permitted actions.
4. **Observe** — The system records task progress, meaningful actions, and page context.
5. **Verify** — It checks whether the requested outcome was achieved.
6. **Report** — The user receives the result, source evidence, and any incomplete steps.

## Core product principles

- **Outcome-oriented:** Users describe what they want accomplished, not every click.
- **User-controlled:** Consequential actions require explicit approval.
- **Observable:** Users can inspect plans, actions, progress, evidence, and verification status without exposing private hidden reasoning.
- **Evidence-backed:** Important claims should link to sources; unknown or unsupported facts must remain clearly marked.
- **Recoverable:** Errors, blocked pages, and incomplete tasks are reported honestly.
- **Progressively autonomous:** Start with bounded research and low-risk workflows; expand autonomy as reliability is demonstrated.

## Planned capabilities

The following describes the product direction, not a claim that every capability is currently implemented.

| Capability | Intended behavior |
|---|---|
| Chat-first task input | Accept natural-language browser tasks |
| Task planning | Break a goal into an understandable sequence of steps |
| Browser control | Navigate and interact with supported websites |
| Execution transparency | Show meaningful task and browser activity in an optional panel |
| Evidence collection | Preserve source URLs and relevant observations |
| Result synthesis | Return structured answers, comparisons, and summaries |
| Verification | Check outcomes instead of trusting an agent's completion claim |
| Approval gates | Pause before consequential actions |
| Task history | Allow users to revisit previous task outcomes |
| Voice interaction | Support spoken task input and responses through an integrated voice service |

## MVP focus

The initial MVP should prove one complete workflow reliably: **multi-site research and comparison**.

Example request:

> Compare five project-management tools for a student team. Check their free plans, collaboration features, and limitations. Give me a comparison table with sources.

A successful run should:

- Create a task plan.
- Search and inspect a defined set of relevant websites.
- Extract the requested facts.
- Preserve source URLs and supporting evidence.
- Mark missing or uncertain information instead of inventing it.
- Produce a structured comparison.
- Show execution progress when transparency is enabled.
- Report completion only after checking the required outputs.

The MVP should prioritize a repeatable end-to-end workflow over broad claims of universal website automation.

## Trust, safety, and privacy

Browsphere is intended to operate in environments where websites may contain sensitive information and actions can have real consequences.

The design therefore calls for:

- Explicit approval before purchases, submissions, messages, account changes, and other consequential actions.
- Stop and cancel controls.
- Least-privilege access to credentials and external services.
- Server-side handling of API keys and secrets.
- Data minimization and clear retention/deletion behavior.
- Separation of trusted instructions from untrusted webpage content.
- Protection against prompt injection in pages and documents.
- Respect for website access controls, rate limits, and applicable terms.
- Honest reporting of failures and incomplete work.

Webpage content is data to inspect, not an authority that can override user intent or system policies.

## Proposed architecture

The system is organized into independently replaceable layers:

1. **Browser client** — Browser UI, chat, task status, results, approval controls, and optional transparency view.
2. **Task orchestrator** — Intent interpretation, planning, task state, scheduling, policy checks, and recovery.
3. **Browser agent** — Page observation, navigation, element interaction, and action results.
4. **Evidence and verification** — Source capture, provenance, outcome checks, and final reporting.
5. **Model and voice services** — Model-provider abstraction for planning/extraction and voice capabilities.
6. **Cloud infrastructure** — Hosting, persistence, secrets, queues, and monitoring as required.

### Task state model

A task may move through states such as:

`CREATED → PLANNING → RUNNING → VERIFYING → COMPLETED`

It may also enter `AWAITING_APPROVAL`, `NEEDS_USER_INPUT`, `BLOCKED`, `FAILED`, or `CANCELLED` as appropriate.

A successful browser action is not, by itself, proof that the user's intended outcome occurred. Verification is a distinct stage.

## Technology integration direction

### AWS

AWS is the proposed cloud infrastructure foundation. Service selection should be driven by the actual deployment needs. Potential responsibilities include application hosting, object storage, task metadata, queues, secrets management, authentication, and operational monitoring.

Candidate services include Amazon S3, Amazon CloudWatch, AWS Secrets Manager, Amazon SQS, Amazon RDS or DynamoDB, and an appropriate compute platform. These are options, not a statement that all are currently deployed.

### ElevenLabs

ElevenLabs is the proposed voice integration for spoken task input and spoken responses, using the specific speech and voice APIs selected during implementation.

Voice should use the same task-planning, authorization, execution, and verification pipeline as typed chat. It must not bypass approval or safety controls.

## Evaluation

The MVP should be measured using repeatable tasks and explicit pass/fail criteria.

| Metric | What it measures |
|---|---|
| Verified task completion rate | Whether the intended outcome was actually achieved |
| Evidence-supported claim rate | Whether returned claims have supporting sources |
| Successful action rate | Reliability of browser interactions |
| Time and actions per task | Efficiency compared with manual completion |
| Trace completeness | Whether meaningful execution events are inspectable |
| Approval-gate compliance | Whether consequential actions are correctly paused |
| Recovery rate | Whether recoverable failures can be handled safely |

Publish measured results with the number of trials, test conditions, failures, and limitations. Do not present unmeasured performance as fact.

## Roadmap

1. **Foundation:** Establish the browser client, chat interface, task model, and backend connection.
2. **Agent execution:** Add planning, page observation, bounded browser actions, cancellation, and error handling.
3. **Evidence and trust:** Add source capture, execution records, verification, approval gates, and prompt-injection defenses.
4. **Integrations:** Connect the required cloud infrastructure and selected ElevenLabs voice capabilities.
5. **Validation:** Run repeatable end-to-end tasks, measure reliability, fix high-impact failures, and prepare the demo.

## Project status

This repository is the project home for Browsphere. The repository's implementation status should be updated as work is delivered and verified.

Use these labels in future documentation:

- **Implemented and verified**
- **Implemented, not fully verified**
- **In progress**
- **Planned**

Do not describe a mockup, design, placeholder, or proposed integration as a completed capability.

## Demo scenario

A concise demonstration should show:

1. A user assigning a multi-site research task in chat.
2. Browsphere generating a plan.
3. The agent navigating real supported pages and collecting evidence.
4. The optional transparency panel showing meaningful execution activity.
5. A structured result with source links and explicit unknowns.
6. A verification step and, where relevant, an approval gate.

The demo should use real, repeatable execution. If a recording is used as a fallback, it should be identified as a recording rather than presented as live activity.

## Contributing and development

Development instructions, supported platforms, setup steps, and test commands will be documented here as the implementation is established. Until those details are verified, they are intentionally not guessed.

## License

No license has been specified yet. All rights are reserved unless and until a license is added to this repository.
