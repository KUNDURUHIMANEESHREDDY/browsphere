# Browsphere

**The browser that works for you.**

Browsphere is an AI-native browser built around a simple idea: you should be able to tell your browser what you want to get done, instead of having to do every step yourself.

Rather than stopping at an answer, Browsphere aims to plan and carry out useful web tasks. It brings together a chat-first experience, browser automation, evidence-backed results, and controls that keep the user in charge.

> **Less time navigating the web. More time getting things done.**

## Why Browsphere?

A lot of everyday work happens in a browser: researching a topic, comparing products, checking information across different websites, or completing a sequence of online steps.

Today, we still do most of that work manually. We open tabs, search, copy information, compare details, and double-check whether we missed something. A chatbot can make this easier by explaining or summarizing, but that does not necessarily mean the task gets completed.

Browsphere is being built to close that gap. You describe the outcome you want, and the browser agent works through the steps on supported websites, keeps track of what it does, and brings the result back to you.

## How it should feel to use

The main way to interact with Browsphere is through chat. You give it a task in plain language, it makes a plan, and then it gets to work in the browser.

If you want to see what's happening, you can open the transparency view. It shows useful execution details such as the current step, browser actions, sources collected, and anything that needs your attention. If you would rather keep things simple, you can hide that view. The agent continues working either way.

### A typical task

1. **You ask:** Describe what you want done.
2. **Browsphere plans:** It breaks the request into steps.
3. **It works:** The browser agent visits supported websites and carries out permitted actions.
4. **It keeps track:** Progress, useful observations, and sources are recorded.
5. **It checks:** The system verifies whether the requested result was actually achieved.
6. **You get the result:** Browsphere shares the outcome, supporting sources, and anything it could not complete.

## What we're building

The following is the intended product direction. It should not be read as a claim that every feature is already implemented.

| Capability | What it means |
|---|---|
| Chat-based tasks | Ask Browsphere to do something using everyday language |
| Task planning | See the steps it intends to take |
| Browser automation | Navigate and interact with supported websites |
| Optional transparency | Inspect meaningful execution activity when you want to |
| Evidence collection | Keep links and relevant information from the pages it visits |
| Research and summaries | Turn collected information into useful answers and comparisons |
| Outcome verification | Check the result instead of assuming an action worked |
| Approval before sensitive actions | Keep the user in control of consequential steps |
| Task history | Revisit previous tasks and their results |
| Voice interaction | Speak requests and hear responses through the selected voice integration |

## The first milestone: research that gets finished

For the first MVP, we're focusing on one useful workflow: researching information across multiple websites and comparing what we find.

For example:

> Compare five project-management tools for a student team. Check their free plans, collaboration features, and limitations. Give me a comparison table with sources.

For this to count as a successful task, Browsphere should:

- Turn the request into a clear plan.
- Visit a defined set of relevant websites.
- Find the information the user asked for.
- Keep the source links and supporting observations.
- Mark information it could not verify instead of making it up.
- Produce a useful comparison.
- Show progress in the transparency view when it is enabled.
- Check that the requested parts of the task are complete before reporting success.

We would rather make one workflow dependable than claim that Browsphere can automate every website from day one.

## Keeping the user in control

A browser agent can encounter private information and actions that have real-world consequences. Trust cannot be an afterthought.

Browsphere is designed around a few important safeguards:

- Ask for approval before actions such as purchases, submissions, sending messages, or changing account settings.
- Let users stop or cancel a task.
- Keep API keys and credentials out of the client wherever possible.
- Give services only the access they need.
- Collect and retain only the data needed to complete the task.
- Treat webpage content as untrusted information, not as instructions that can override the user's request.
- Respect website access controls and rate limits.
- Be clear when a task fails, gets blocked, or is only partly complete.

A webpage should never be able to persuade the agent to reveal secrets or ignore the user's instructions.

## How the system fits together

Browsphere is made up of a few parts that work together:

1. **Browser experience** — The browser interface, chat, task progress, results, and approval controls.
2. **Task orchestrator** — Understands the request, plans the work, tracks progress, and decides what should happen next.
3. **Browser agent** — Observes pages, navigates websites, and performs permitted interactions.
4. **Evidence and verification** — Keeps track of sources and checks whether the requested outcome was reached.
5. **AI and voice services** — Support task planning, information extraction, and spoken interaction.
6. **Cloud infrastructure** — Supports the backend, storage, queues, secrets, and monitoring where needed.

A task might move through:

`CREATED → PLANNING → RUNNING → VERIFYING → COMPLETED`

Some tasks may pause for approval or user input. Others may be blocked, fail, or be cancelled. A click succeeding does not automatically mean the user's goal has been achieved, so verification is a separate step.

## Why AWS?

AWS is intended to provide the cloud foundation behind Browsphere. It is not the part that decides what the user wants or controls the browser; it provides the backend services that help the application run reliably.

For example, when someone gives Browsphere a research task, the backend needs to create a task record, keep track of its progress, store useful files, protect credentials, and make it possible to investigate errors. AWS services can take care of those infrastructure responsibilities so the product can focus on completing the task.

Depending on the needs of the implementation, the AWS pieces could include:

| AWS service | Where it could help |
|---|---|
| Amazon S3 | Store generated reports and other task files |
| Amazon SQS | Queue work that should run in the background |
| Amazon DynamoDB or Amazon RDS | Keep task records, status, and related metadata |
| AWS Secrets Manager | Store service credentials securely on the backend |
| Amazon CloudWatch | Collect logs and help investigate errors and performance |
| Amazon ECS / Fargate, EC2, or another suitable compute option | Run backend services and agent workers |

We do not need every service just because it is available. The final architecture should use only the AWS services that solve a real requirement for the MVP. The table above describes the intended options; it does not mean they are all deployed today.

## Why ElevenLabs?

We want Browsphere to feel natural to use, whether someone prefers typing or speaking. ElevenLabs is intended to power the voice side of that experience.

A user could speak a request instead of typing it. Browsphere would turn that request into the same task flow, use the browser agent to do the work, and then present the result on screen. If the user wants, it could also read the result or task updates aloud.

Depending on the ElevenLabs products and APIs selected, this could include:

- **Speech input:** Turn a spoken request into text the task system can understand.
- **Text-to-speech:** Read answers, summaries, or progress updates aloud.
- **Streaming voice:** Make spoken interactions feel more responsive.
- **Voice design or selection:** Give the product a consistent voice.

The important part is that voice does not become a separate agent with different permissions. Whether a task comes from typed chat or speech, it goes through the same planning, safety checks, browser execution, and verification. Speaking a request must never bypass an approval step.

These are the intended integration roles. The exact ElevenLabs APIs and the features connected to them should be documented as they are implemented and tested.

## How AWS, ElevenLabs, and the agent work together

Think of the responsibilities this way:

- **Browsphere is the product:** it owns the browser experience, task flow, permissions, and results.
- **The AI model helps understand and plan:** it interprets requests, structures tasks, and helps process information.
- **The browser agent does the web work:** it navigates pages and performs allowed actions.
- **AWS supports the backend:** it can host services, store task data and files, manage background work, and provide monitoring.
- **ElevenLabs adds voice:** it can let users speak to Browsphere and hear responses.

For a spoken research request, the flow would look like this:

1. The user speaks a request.
2. The selected ElevenLabs speech service converts the audio into text, where that capability is used.
3. Browsphere's task system plans the request and applies its normal safety rules.
4. The browser agent visits the relevant websites and gathers information.
5. The backend uses the selected AWS services to manage task state, background work, and stored artifacts.
6. Browsphere checks the collected information and prepares the result.
7. The result appears in the browser and can optionally be spoken using ElevenLabs text-to-speech.

AWS and ElevenLabs support the experience; they do not replace Browsphere's own orchestration, browser agent, or verification logic.

## How we'll know it's working

We want to measure whether Browsphere actually helps people finish tasks, not just whether it can produce an impressive-looking demo.

| Measure | What we'll look at |
|---|---|
| Verified task completion | Did the task reach the outcome the user asked for? |
| Evidence-backed claims | Can important statements be traced to sources? |
| Browser action reliability | How often do the required browser steps work? |
| Time to complete | Does delegation save time compared with doing it manually? |
| Execution visibility | Can users understand what happened when they choose to inspect it? |
| Approval behavior | Does the agent pause when a consequential action needs permission? |
| Failure recovery | Can it explain and recover from common problems? |

We'll report the test conditions, number of runs, failures, and limitations alongside any results. We won't present targets or guesses as measured performance.

## Roadmap

1. **Get the foundations in place:** Build the browser experience, chat, task model, and backend connection.
2. **Make the agent useful:** Add task planning, page observation, supported browser actions, and reliable error handling.
3. **Make results trustworthy:** Add evidence capture, verification, approval gates, and protection against malicious webpage instructions.
4. **Connect the services:** Integrate the AWS infrastructure and the specific ElevenLabs voice capabilities the product needs.
5. **Test real tasks:** Run repeatable workflows, measure what works, fix the weak points, and prepare a reliable demo.

## Project status

This repository is the home for Browsphere. We will keep implementation status honest as the project develops.

Use these labels when documenting features:

- **Implemented and verified**
- **Implemented, not fully verified**
- **In progress**
- **Planned**

A mockup, placeholder, or proposed integration should never be described as a completed feature.

## Demo idea

A short demo should follow one task from beginning to end:

1. The user asks Browsphere to research and compare several tools.
2. Browsphere creates a plan.
3. The agent visits real websites and collects information.
4. The transparency view shows what it is doing, and can be hidden to show the simpler experience.
5. The final comparison includes source links and clearly marked unknowns.
6. Browsphere verifies the requested information and shows any step that could not be completed.

The demo should use real, repeatable execution. If a recording is used as a fallback, it should be clearly presented as a recording.

## Development

Setup instructions, supported platforms, architecture details, and test commands will be added as they are confirmed in the repository. We don't want to invent commands or prerequisites that have not been tested.

## License

No license has been selected yet. Until one is added, all rights are reserved.
