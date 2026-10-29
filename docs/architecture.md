# Architecture

Weksa is a language-intent engine: currently a state spine, pipeline map,
fixtures, ontology data and a planned service contract. No daemon runs; the
planned packet crate is `crates/weksa` (cut `packet-store`).

See [Verse Service Contract](./verse-service-contract.md) for the
CultCache/CultMesh authority map.

## Objective

Build a procedural alien-language engine whose outputs are grounded in
worldview, semantics, grammar, phonology, morphology, and history. Once a service is
earned (planned), Weksa also turns writing plus agent state into typed conversational intent, then
lowers that intent into written realizations, pronunciation plans, speech-provider
requests, and utterance handoff documents.

## Current Mechanism

There is no long-running runtime loop yet. The repo contains persistent state
surfaces and a provider advertisement fixture that define the intended data flow
and the ownership boundaries future code must respect.

The first shared contract is the [Interlingua Standard](./interlingua-standard.md):
clients submit interlingua packets before any language project projects them
through its ontology.

## Invariants

- Meaning precedes words.
- Ontology owns concept distinctions.
- Grammar owns obligatory expression.
- Morphology owns form families.
- Phonology owns sound legality.
- Diachrony owns plausible irregularity.
- Renderer owns output, not interpretation.
- Weksa owns conversational intent and utterance lowering.
- External speech providers own rendered audio, not meaning or delivery truth.
- AquaSynth owns learned utterance embeddings, synth controls, Faust
  compilation, and audio output.

## Intended Change

Future implementation should turn the pipeline in `state/map.yaml` into a small,
inspectable daemon. The first executable slice should validate a tiny
interlingua packet, read a tiny language project definition, render glossed
output, lower an accepted intent into target-language realizations, optionally
project the accepted realization into a speech-provider request or
pronunciation/utterance handoff, and produce a trace explaining which stage made
each decision.

Runtime should be fast and deterministic. Model-assisted English decomposition
belongs in authoring tools or offline import flows, not in the required hot path.
See [Runtime and Authoring Split](./runtime-authoring-split.md).

## Cut Line

Do not add a database, queue, plugin system, prompt framework, or adapter until
the pipeline stage needing it is known.

Do not hard-code Rust to Dust, its aliens, or its aesthetic-honor ontology into
the generic engine.
