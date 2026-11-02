# BreadShot roadmap

BreadShot is building the zero-edit screen recorder for Windows: press one key, demonstrate naturally, and receive a polished local MP4 without opening a timeline.

Our long-term public goal is to build a product and open-source community worthy of at least 10,000 genuine GitHub stars. Stars are an outcome, not the product metric: no purchased stars, automated accounts, giveaways that require starring, or misleading promotion.

## North-star product test

A first-time user records a 60-second software demonstration and is willing to publish BreadShot's automatic output without manually editing it.

We will measure:

- **No-edit success rate:** at least 90% across the maintained interaction benchmark.
- **Recording reliability:** at least 99 successful capture-and-export runs out of 100 standard runs.
- **Bad-camera rate:** fewer than 1 visibly wrong zoom or camera movement per 10 minutes.
- **First-result time:** a new user can install, record, and obtain a result without documentation.
- **Privacy:** recording and rendering remain usable without an account or upload.

## Milestone 1 — Automatic Director 2.0

The next release must make the automatic output meaningfully different from ordinary click-to-zoom recorders.

- Build a versioned benchmark of difficult mouse and UI interactions.
- Distinguish click, double-click, drag, scroll, typing activity, dwell, and high-speed search movement.
- Group related actions into stable intent segments.
- Delay camera decisions when the interface is likely to change after an action.
- Add confidence scoring and prefer no zoom over a wrong zoom.
- Prevent oscillation, excessive cuts, edge clipping, and motion sickness.
- Compare every algorithm change against the complete benchmark.

**Exit gate:** 18 of 20 benchmark scenarios are publishable without camera correction, with no regression in the remaining scenarios.

## Milestone 2 — Reliable capture

- Window capture.
- Region capture.
- Long-recording and low-disk-space safeguards.
- Recovery of usable source media after an interrupted session.
- Accurate processing stages and actionable failures.
- Compatibility coverage across common Intel, AMD, and NVIDIA Windows machines.

**Exit gate:** 100-run capture/export test with at least 99 successful final videos and a recoverable source for every failed render.

## Milestone 3 — Automatic composition

- Local automatic backgrounds, padding, corners, and framing.
- Output presets for 16:9, 9:16, and 1:1.
- Composition rules that react to the captured window and focus target.
- A small number of excellent defaults instead of a large manual editor.

**Exit gate:** blind comparison participants can reliably distinguish the BreadShot result from the raw recording and prefer it for publishing.

## Milestone 4 — Public-launch quality

- Signed or clearly verifiable Windows releases.
- Automatic update checking.
- A 20-second raw-versus-BreadShot hero demonstration.
- English and Chinese documentation with reproducible setup and troubleshooting.
- Contribution guide, architecture entry point, issue templates, Discussions, and scoped good-first issues.
- A public compatibility matrix and known-limitations list.

**Exit gate:** a first-time user can discover, download, install, record, export, and report a problem without private help from the maintainer.

## Distribution ladder

### 0 → 1,000 stars: prove the result

- Lead with one undeniable before/after demonstration.
- Recruit users from Windows, developer-demo, education, and open-source communities.
- Fix installation and failed-recording issues before adding broad features.
- Publish benchmark improvements with real video evidence.

### 1,000 → 3,000 stars: prove the project

- Maintain a predictable release rhythm.
- Respond to reproducible issues quickly.
- Publish architecture and automatic-director design notes.
- Create small, tested contribution surfaces.
- Feature real videos made by users, with permission.

### 3,000 → 10,000 stars: establish the category

- Publish a technical launch explaining semantic automatic direction.
- Release the interaction benchmark so improvements are measurable.
- Coordinate separate launches on Show HN, Product Hunt, Reddit, V2EX, Bilibili, and creator channels instead of one simultaneous blast.
- Evaluate macOS and Linux only after the Windows experience and engine boundaries are proven.
- Let the community contribute tested director rules before considering a general plugin marketplace.

### 10,000+ stars: expand the platform

- Optional local captions and speech cleanup.
- Automatic webcam composition.
- Cross-platform capture adapters.
- A stable director-rule or extension API.
- Optional sharing services that do not compromise the local-first core.

## Deliberate non-goals for now

- A general-purpose manual video editor.
- Cloud accounts, team workspaces, and storage.
- A plugin marketplace before a stable extension contract exists.
- Feature-count competition with OBS, Camtasia, Recordly, Cap, or Loom.
- Growth tactics that inflate stars without creating active users or contributors.

## Decision rule

Every proposed feature must improve at least one of these:

1. automatic output quality;
2. successful recording and export rate;
3. time to first publishable video;
4. trustworthy open-source adoption.

If it improves none of them, it is not a current BreadShot priority.
