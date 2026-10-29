# Weksa

Weksa is a procedural language engine for alien grammars grounded in concept,
culture, and constraint, and the planned lowerer that turns an agent's
meaning packet into the reader's display language.

**What is valuable here.** The interlingua standard and the lowering design: a
meaning packet owns what is meant, a target-language profile owns register,
address and politeness, and a lowerer owns surface text and leaves a trace.

**The problem it solves.** An agent's outward speech today is one English
string the model chose, so meaning, register and voice are fused and nobody can
inspect or re-target it. Packets separate them, so the same meaning can be
lowered into English, other languages and conlangs, and the loss can be
measured.

**Entry points.**

- [Interlingua Standard](./docs/interlingua-standard.md): the packet contract.
- `crates/weksa`: the packet, inventory and corpus types, `validate`, the
  derived split and the `weksa` CLI (`weksa validate`, `corpus`, `entity`, `inventory`,
  `split`, `schema`); published schemas in `schemas/cultnet/`.
- `data/corpus/corpus.cc`, `data/entity/entity.cc` and `data/interlingua/inventory.cc`: the English
  corpus, its entities and the inventory, read with `weksa corpus dump`,
  `weksa entity dump` and `weksa inventory dump`.
- [Lowering target](./docs/targets/interlingua-lowering.md) and
  [its map](./docs/targets/interlingua-lowering-map.md): the design, scoring
  and cut sequence.
- `data/`: the cultural ontology and target-language profile data.
- `settings/`: setting-specific proving grounds for language projects.

**Live versus planned.** Live: docs, fixtures, examples, ontology data and the
packet crate `crates/weksa`. Planned: scoring, lowering, and
the golf loop that scores lowered English against an expressive corpus. No
daemon runs; the earlier Node daemon is parked at tag `parked/weksa-node-body`.

## Ownership Model

- Ontology owns meaning.
- Grammar owns required expression.
- Phonology owns sound shape.
- Diachrony owns historical distortion.
- Renderer owns surface text.
- Persistent state owns durable maps, constraints, decisions, and evidence.

If a future module cannot state what it owns and what invariant it protects, it
does not get to exist. Harsh, but the alternative is a language generator with a
clipboard and delusions of nationhood.

## Scaffold

- `state/map.yaml`: canonical working map.
- `state/scratch.md`: disposable workbench.
- `schemas/`: human-facing state contract receipts.
- `docs/`: architecture, language model, and roadmap.
- `settings/`: setting-specific proving grounds for language projects.

Start with [Best Modern Approach](./docs/best-modern-approach.md) for the core
design doctrine: ontology, roots, morphology, syntax, semantics, interlingua,
diachrony, and model-assisted consistency.

See [Project Model](./docs/project-model.md) for the engine / language project /
setting split, and [Runtime and Authoring Split](./docs/runtime-authoring-split.md)
for the LLM-free hot path.

See [Interlingua Standard](./docs/interlingua-standard.md) for the shared
meaning-packet contract Weksa clients should target.

See [Verse Service Contract](./docs/verse-service-contract.md) for the planned
service contract: Weksa would own typed conversational intent, pronunciation
plans, utterance handoffs, and the typed CultMesh state that makes those
documents inspectable. No daemon runs yet.

See [Multilingual Utterance Lowering](./docs/multilingual-utterance-lowering.md)
for the shared lowering target: interlingua projected through speaker state into
English, Brazilian Portuguese, Japanese, and later target-language dialogue.
[Flavored English Lowering](./docs/flavored-english-lowering.md) remains the
target-specific English receipt.

See [Target-Language Cultural Ontology Profiles](./data/target-language-ontology/README.md)
for the reviewable address, register, politeness, idiom, metaphor, and script
distinctions that target lowerers must inspect before producing output.

For the first AquaSynth speech-training handoff, see
[Tiny PanPhon v0.1 Speech Curriculum](./examples/speech-training/tiny-panphon-v0.1/README.md).
Those packets are emitted by `tools/emit_tiny_panphon_handoffs.py` and provide
the crawl-stage artifact for simple IPA sounds. Validate the checked contract
with `tools/validate_tiny_panphon_handoffs.py` before handing the batch to
AquaSynth or expanding pronunciation coverage.

## Implementation Status

The persistent state spine is live: map, scratch, schemas, docs, examples and
data. Packet types, validation and the corpus and inventory stores are live in
`crates/weksa`. Scoring, lowering and the service are planned (see the cut
sequence in [the lowering map](./docs/targets/interlingua-lowering-map.md)).
Writing and agent state will enter Weksa as intent pressure, and typed utterance
documents will leave for AquaSynth.

[Eusocial Interbeing / Zyphos](./settings/eusocial-interbeing/README.md) is the
current higher-priority setting target. [Rust to Dust](./settings/rust-to-dust/README.md)
is preserved as an earlier proving ground, not engine doctrine.
