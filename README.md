# Weksa

Weksa is a procedural language engine for alien grammars grounded in concept,
culture, and constraint.

The project starts from a blunt rule: English words are not the source of truth.
Meaning is decomposed into an ontology, passed through a worldview, constrained
by grammar, shaped by phonology, distorted by history, and only then rendered as
surface text.

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

See [Verse Service Contract](./docs/verse-service-contract.md) for the daemon
contract: Weksa owns typed conversational intent, pronunciation plans, utterance
handoffs, and the typed CultMesh state that makes those documents inspectable. Eve is deprecated and Thing 2 is its successor; the operator Eve surface is quarantined (see the contract).

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

The first committed artifact is the persistent state spine: map, scratch,
schemas, docs, examples, and a read-only provider advertisement fixture. The
daemon runtime still needs implementation, but its authority is no longer
ambiguous: writing and agent state enter Weksa as intent pressure, and typed
utterance documents leave for AquaSynth.

[Eusocial Interbeing / Zyphos](./settings/eusocial-interbeing/README.md) is the
current higher-priority setting target. [Rust to Dust](./settings/rust-to-dust/README.md)
is preserved as an earlier proving ground, not engine doctrine.
