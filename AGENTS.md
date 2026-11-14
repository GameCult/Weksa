# Weksa Instructions

Weksa follows the global coherence rule: map first, cut cleanly, build only the
parts whose authority is clear.

Before substantial implementation, state:

- Objective
- Current mechanism
- Invariants
- Intended change
- Cut line

## Project Doctrine

- Do not translate English directly into alien words.
- Decompose source text into meaning first.
- Let the alien ontology decide which distinctions split, collapse, or become
  obligatory.
- Keep neural models as semantic decomposition, generation, and critique tools.
- Keep linguistic authority in explicit tables, typed state, and deterministic
  renderers.
- Do not build rules-based language cops for fuzzy meaning. Use model-based
  classification or trainable readers for semantic interpretation; reserve
  hand-written rules for deterministic grammar and validation.

## Ownership

- Ontology owns concept identity, relations, domains, salience, and worldview
  distortions.
- Grammar owns required expression, syntax, inflection, agreement, and clause
  structure.
- Phonology owns legal sounds, syllable shape, prosody, phonotactics, and sound
  symbolism constraints.
- Morphology owns derivation, inflection, compounding, and root-family behavior.
- Diachrony owns historical sound change, semantic drift, register split, loan
  strata, and irregularity.
- Renderer owns surface forms from already-structured meaning.
- Persistent state owns maps, accepted design, invariants, evidence, scratch,
  and durable decisions.

## Persistent State

The live state architecture is ported from EpiphanyAgent in reduced form:

- `state/map.yaml` is canonical and slow-changing.
- `state/scratch.md` is temporary and disposable.
- `schemas/` explains state contracts in human language.
- `crates/weksa` owns executable typed state.

Project truth does not belong in agent personality memory. If Weksa later grows
role dossiers or heartbeat scheduling, those surfaces must remain separate from
language-project truth.

## Building And Verifying

- `crates/weksa` depends on CultLib's `cultcache-rs` as a git dependency pinned by
  `rev`, fetched from GitHub. With the default `CARGO_HOME` that fetch fails on a
  Mind's host. Use the pre-fetched home:
  `CARGO_HOME=/var/lib/mind-weksa-self/cargo-home cargo test --workspace`.
- `weksa packet proof <store>` admits the nibu proof packet under the key
  `packet:6e6962752d303031` (hex of `nibu-001`), not `nibu-proof-001`. Use that key
  with `weksa pronounce <store> <key>`.
