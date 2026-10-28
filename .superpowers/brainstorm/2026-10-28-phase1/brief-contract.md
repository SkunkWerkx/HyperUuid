# Design-brief contract (Phase 1 research, 2026-10-28)

You are researching one ROADMAP Phase 1 item for HyperUuid (/Users/buvy/Developer/SkunkWerkx/HyperUuid). HyperUuid is one Rust RFC 9562 UUID core reached from nine language bindings. You produce a **design brief**, not a spec and not code. The owner approves the design before anyone writes a spec, so the brief has to make their decisions easy.

## Hard rules
- **Read-only.** Edit nothing in any repo and make no git writes. The only file you write is your brief. You may install nothing, or install under `~` only if a probe needs it. Don't dispatch subagents.
- **Read first:** `CLAUDE.md` (all of it, especially "Lists that move together", "Toolchains and floors" and "Forge coupling and releases"), `ROADMAP.md` (Standing policies and Phase 1), and the binding's own README. The Forge lives at `../HyperForge` (`.github/workflows/`, `.github/actions/`, `scripts/hyper_config.py`). The sibling repos are `../HyperCast` and `../HyperTabular`. CLAUDE.md says floors match across all three repos and move together.
- **Check facts.** Every upstream fact (EOL dates, SDK or JDK behaviour, registry rules, toolchain support) must be checked today against a primary source, such as endoflife.date, vendor docs or the registry's docs. Cite the URL. Mark anything you could not verify **UNVERIFIED**. Don't trust the ROADMAP's own claims; check them.
- **Measure from the repo.** Every claim about the code gets a `file:line`.

## Brief format (write it to the path in your dispatch)
1. **Goal**, in one paragraph: what changes for a user, and why now.
2. **Current state:** how this binding builds, tests, packs and publishes today, with file:line references, including the Forge pieces.
3. **Upstream facts:** each with its source URL and the date checked.
4. **Approaches (2–3):** how each works, its trade-offs and its cost. End with your recommendation and why.
5. **Touch list** for the recommended approach. Every file in this repo, in the Forge and in each sibling, marked:
   - `[own]` for files only this item touches;
   - `[shared]` for CHANGELOG, root README, ROADMAP, CLAUDE.md, `ci.yml`, Forge files used by other bindings, the C-export lists, `.github/hyper.json`, and anything else another Phase 1 item might also touch.

   The `[shared]` marks decide what can run in parallel, so err toward marking.
6. **Sibling cascade:** what HyperCast and HyperTabular must change to keep floors matched, and whether they even have this binding.
7. **Verification:**
   - what proves it locally, with commands, and the negative control for each check;
   - what only CI can prove;
   - any release-time proof, for example a first publish of a new artifact shape.
8. **Risks and unknowns,** ranked.
9. **Owner questions:** at most 5, each multiple-choice with your recommended option first. These are only the decisions you cannot make from the code and the docs.

Keep the brief under about 250 lines. Reply to your dispatcher in under 12 lines: your recommendation, the count of `[shared]` touches, the top risk and the brief's path.
