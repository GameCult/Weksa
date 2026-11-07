# Target-Language Cultural Ontology Profiles

These profiles describe culturally loaded realization choices for human target
languages used by Weksa utterance lowering.

They are not whole-culture summaries. They are bounded lowering profiles: the
distinctions a target lowerer must inspect before producing dialogue.

## Authority

Target-language cultural ontology profiles own:

- address and politeness distinctions
- register and social stance pressures
- agency, responsibility, and directness defaults
- idiom and metaphor risks
- script or orthographic choices that carry meaning
- forbidden flattenings that would make the output dishonest

They do not own:

- source meaning
- speaker-private state
- target-language grammar as a whole
- national or cultural stereotypes
- final wording without an interlingua packet

## Current Profiles

- [English, US fixture default](./en-US.yaml)
- [Brazilian Portuguese](./pt-BR.yaml)
- [Brazilian Portuguese overlay catalog](./pt-BR-overlays.yaml)
- [Brazilian Portuguese, Umbanda overlay](./pt-BR-umbanda.yaml)
- [Japanese](./ja-JP.yaml)
- [Rust to Dust imperial (DRAFT placeholder, JSON so tests can read it)](./rtd-imperial.json), checked by `crates/weksa/tests/rtd_slice.rs`

Each multilingual lowering fixture should cite the profile or profile stack it
used so reviewers can tell which social and cultural distinctions were
considered. Regional and religious-cultural options are overlays on a base
language profile, not replacements for source meaning.
