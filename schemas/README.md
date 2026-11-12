# Schemas

This folder records Weksa's persistent state contracts in human language.

The repo deliberately starts without general executable schema code. Runtime
language, storage backend, and generated type targets are undecided. Until those
decisions are made, this folder is the receipt surface: if a state shape matters
enough to steer the machine, describe its authority here.

Narrow fixture validators may exist when an external handoff needs an executable
proof. They do not become the schema authority unless the map explicitly promotes
them.

## Canonical Surfaces

- [project-state.md](./project-state.md): project map, scratch, decisions,
  evidence, and open questions.
- [language-project-state.md](./language-project-state.md): future per-language
  state for ontology, grammar, morphology, phonology, diachrony, and renderer
  configuration.
- [interlingua-packet.md](./interlingua-packet.md): shared meaning packet
  contract between clients, authoring tools, and language projects.
- [utterance-lowering-packet.md](./utterance-lowering-packet.md): shared packet
  contract for lowering interlingua and projected speaker context into one or
  more target-language utterance realizations.
- [cultural-ontology-profile.md](./cultural-ontology-profile.md): nested
  CultCache profile contract for cultural ontology, including typed salience
  axes, linguistic affordance feature records, evidence, activation gates, and
  lowering traces.
- [english-lowering-packet.md](./english-lowering-packet.md): packet contract
  for the older English-specific realization lane.
- [english-pronunciation-plan.md](./english-pronunciation-plan.md): plan for
  stringing flavored English into IPA/phoneme sequences before downstream speech.
- [utterance-embedding-handoff.md](./utterance-embedding-handoff.md): learned utterance embedding handoff
  contract for handing prosody, emphasis, and character pressure to downstream
  synth control.

## Source Of Truth

- `state/map.yaml` is the canonical live map for this workspace.
- `state/scratch.md` is temporary working memory.
- `schemas/` explains the contracts.
- Future executable schemas must be generated from or reconciled with these
  receipts in the same pass that introduces them.
