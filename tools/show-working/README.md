# Show her working

A static page that shows the lowering chain for committed fixture packets: typed slots, inventory entries chosen, lowered English, scorer closeness to the corpus target, with hedge, certainty and restraint annotated. Refusals (admission rules, `LowerError`, addressee consent, negation awaiting polarity) are results, not omissions.

- `crates/weksa/examples/show_working.rs`: runs the real admission door, `lower` and `pronounce` over `proof_packet()` and its variants and prints JSON. Each feature is removed in turn and the packet re-run, so "the lowerer read this" is measured.
- `build.py`: runs that, scores each lowered text with the `weksa score` CLI (needs Ollama with `qwen3-embedding:0.6b`), writes `out/data.json` and one dependency-free `out/index.html`. Fails if anything cannot run; there is no fallback text.
- `scripts/publish-show-working.sh`: build, then publish through gamecult-ops `publish-mind-preview.sh`.

Adding a fixture: add an entry in `fixtures()` in the example. A packet from another store needs a loader there; none exists yet.
