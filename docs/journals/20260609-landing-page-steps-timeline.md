# Landing Page: Four-Steps Timeline Refinement

**Date**: 2026-06-09 
**Severity**: Low
**Component**: Landing page UI (docs/index.html), "Up and running in four steps" section
**Status**: Completed

## What Happened

Iterated landing page layout for the four-step setup guide. Changes focused on visual clarity, copy tightness, and install-command readability.

## The Brutal Truth

Early iteration attempts (circular grid layout with floating arrow badges) felt cluttered and over-engineered for what should be a simple vertical flow. Went full circle back to a clean vertical timeline—simple, readable, no cognitive load. Also cut bloated step descriptions by 70%+ because every sentence was tentative filler explaining edge cases nobody reads anyway.

## Technical Details

**Install command wrapping:** Changed `.cmd code` from `white-space:nowrap; overflow-x:auto` to `white-space:normal; overflow-wrap:anywhere; word-break:break-word`. Moved `.cmd` alignment from `center` to `flex-start`. Result: long URLs wrap cleanly instead of forcing horizontal scroll—critical on mobile.

**Timeline visual:** Added continuous vertical rail (`.step:not(:last-child)::before`, 2px border, height calc(100% + 18px)) linking numbered nodes. Numbered nodes (`.step .num`) changed from 10px border-radius (square) to 50% (circle), added `box-shadow:0 0 0 4px var(--bg)` to punch through the rail cleanly at each node without visual overlap.

**Layout tightening:** `.steps` gap reduced 16px → 18px, padding/sizing adjusted to accommodate new z-indexing for the rail. `.step .num` now `position:relative; z-index:1` to sit atop the pseudo-element.

**Copy edits (examples):**
- Removed kicker ("Get started") label entirely.
- Step 1: "One command on Ubuntu 22.04+ — downloads the prebuilt binary into `~/.local`, no build and no Rust toolchain" → "One command on Ubuntu 22.04+ — prebuilt binary, no build."
- Step 2: Removed entire paragraph about AppIndicator extension and config backup mechanics; replaced with "Then run `agentpet` or launch from your app menu."
- Step 3: Consolidated 3 sentences about Petdex into 1 + command.
- Step 4: Removed detailed mood explanation; now: "In Settings → Pet, assign a pet to each agent. It appears when that agent is active — its mood mirroring the live state: calm, busy, or waving."

**Activities label:** Enlarged topbar `.activities` label (font-weight:500 → 600, font-size:14px) for visual emphasis in the simulation scene.

## What We Tried

1. **Circular numbered nodes with per-gap connector lines** — Too visually busy, unclear hierarchy.
2. **2×2 grid with floating arrow badges** — Looked modern but confusing on narrow viewports and felt like novelty over clarity.
3. **Vertical timeline with continuous rail** — Landed on this. Clean, scannable, guideline-like guidance without gimmickry.

## Root Cause Analysis

Tendency to over-design simple hierarchical structures. A vertical timeline is pedestrian, which is exactly why it works—users scan top-to-bottom, rail anchors their eye, nodes are instantly clear. Earlier attempts prioritized "interesting" over "functional."

Copy bloat stemmed from defensive writing (explaining every safety measure, fallback, optional parameter). Real users skim. Led copy should answer "what do I do next?" not "what happens if I deviate?"

## Lessons Learned

- **Simplicity scales:** vertical timeline requires zero layout query / grid breakpoint logic. Works on 320px and 1200px without tricks.
- **Copy scarcity breeds clarity:** forced word cuts (from ~180 words across 4 steps to ~90) forced elimination of hedging language. Remaining text is stronger.
- **Visual iteration cost:** three layout attempts consumed mental effort. Quick wireframe/ASCII mockup of timeline + rail upfront would have collapsed the decision space by step 2.
- **Command wrapping != horizontal scroll:** wrap cost browser real estate but zero cognitive load. Scroll forces extra interaction layer. Wrap wins even on desktop.

## Next Steps

- Commit landing page changes.
- Monitor in-flight Rust changes (crates/agentpet/src/{pet,ui}/*.rs, assets/agents/) — not part of this session but staged. Verify compile + test before merge.
- Future: A/B test copy length (current vs. original) if we track bounce/scroll metrics.

## Unresolved Questions

None—design decision locked, copy finalized, command wrapping tested manually on responsive widths.
