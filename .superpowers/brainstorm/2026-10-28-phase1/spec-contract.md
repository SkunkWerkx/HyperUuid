# Spec-writing contract (Phase 1, 2026-10-28)

You turn one approved design into a written spec for HyperUuid (/Users/buvy/Developer/SkunkWerkx/HyperUuid). Your inputs are a design brief written by a researcher and the owner's decisions.

## Authority, highest first
1. `.superpowers/brainstorm/2026-10-28-phase1/decisions.md`. The owner's decisions are binding. Where the brief disagrees, the decisions win: siblings frozen, net10 scope, Apple exclusion, release vehicle, names and so on.
2. The repo's `CLAUDE.md`, except "floors move together" and the sibling cascade, which are suspended until v1.1 ships.
3. Your item's design brief. It is a researcher's work: re-check any `file:line` or upstream fact you rely on, and fix it if it is wrong.

## Hard rules
- Write exactly one file, the spec path in your dispatch. Edit nothing else and make no git writes. Don't dispatch subagents. Install nothing unless a fact can't be settled any other way.
- **No sibling work.** HyperCast and HyperTabular are frozen until HyperUuid v1.1 ships. List nothing for them; at most, one line under "Later" for their future roadmaps.
- **Everything ships in 1.1.0,** with the v7 re-seed and the first npm and CRAN publish.
- **Guiding criterion: ubiquity, and nitpick-proof under scrutiny.** Every claim the change makes in a README must be proven by a named check, or labelled as unproven. The spec has a section listing where a hostile reviewer could still poke a hole and how each is closed or labelled.
- **CLAUDE.md rules that bind specs:**
  - Claims are measured: dated, one machine, losses beside wins.
  - Every verification has a negative control.
  - READMEs state the current state only.
  - Install snippets keep `<version>`.
  - Third-party actions are pinned to a SHA.
  - In the Forge's `hyper-build-native.yml`, a new binding's steps go first.
  - Lists that move together move together.

## Format
Match the style of `docs/superpowers/specs/2026-10-27-v7-counter-reseed-design.md`. Read its first 80 lines for tone and structure. Sections:
1. **Title, date and status** ("Draft for owner review").
2. **Goal:** what reach this buys, in one paragraph.
3. **Decisions:** the owner's, quoted or summarised, each with its date.
4. **Design:** exact names, files, layouts and behaviour. A spec says what, not how-to-code.
5. **Contract changes:** the README, CHANGELOG and doc wording, written as they should finally read.
6. **Files:**
   - `[own]` and `[shared]` lists, with the Forge separate.
   - Each `[shared]` file says exactly which lines or section this item changes, so four items can be integrated without conflicts.
7. **Ordering constraints:** what must merge first, such as Forge before repo.
8. **Verification:**
   - locally, each check with its negative control;
   - CI-only, with the job names;
   - release-time, meaning the first publish of a new artifact shape.
9. **Nitpick audit:** each hole a reviewer could find, and how it is closed or labelled.
10. **Out of scope / Later.**
11. **Open items,** if any remain. Keep them few, each with a recommended answer.

Keep it under about 250 lines.

## Before you finish
Self-review for placeholders, contradictions with decisions.md and ambiguity. Fix them inline. Then reply in under 10 lines: the spec path, its line count, the number of `[shared]` touches, any open items, and any brief claim you found wrong.
