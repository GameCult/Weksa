# Multilingual Utterance Lowering

Version: `0.1-draft`

Weksa can lower one structured utterance intent into multiple human-language
realizations and spoken-provider requests without making English the hidden
source of truth.

```text
interlingua packet
  -> projected speaker-local context
  -> utterance lowering request
  -> target-language realization packets
  -> optional speech-provider request projections
```

The first targets are English, Brazilian Portuguese (`pt-BR`), and Japanese
(`ja`). These targets prove the middle of the machine before alien-language
rendering gets to hide failures under unfamiliar surface forms.

## Objective

Produce target-language dialogue from a shared meaning packet while preserving:

- source referents and predications
- speaker-local knowledge boundaries
- target-language cultural ontology
- target-language register and obligatory expression
- traceability from output back to interlingua and projected context

## Authority

The interlingua packet owns:

- what is meant
- referents, predications, relations, discourse, constraints, and provenance
- source-side uncertainty

The projected speaker context owns:

- what the speaker knows, believes, remembers, wants, and can express
- voice pressure, relationship stance, embodiment, and presentation limits

The utterance lowerer owns:

- selecting target-language surface text
- preserving required meaning and constraints
- obeying speaker-local context
- applying the target-language cultural ontology profile
- marking target-language register and realization choices
- emitting backglosses, backtranslations, and lowering trace

The utterance lowerer does not own:

- new facts
- raw agent-state internals
- target-language alien forms
- rendered audio
- phoneme parity or synth-control truth

The target-language cultural ontology profile owns:

- address, politeness, register, and social stance distinctions
- idiom and metaphor risks
- script or orthographic choices that carry meaning
- forbidden flattenings for that target

It does not own source meaning. It tells the lowerer what distinctions must be
inspected before surface text is chosen.

## Target Languages

### English

English remains a lowering target, not the pivot language. Existing flavored
English fixtures continue to be useful, but new multilingual fixtures should
show English as one realization in a set.

The initial cultural ontology profile is
`data/target-language-ontology/en-US.yaml`.

### Brazilian Portuguese

Portuguese must be explicit about locale. The initial target is `pt-BR`, not a
generic Portuguese bucket.

The target lowerer should record:

- `locale: pt-BR`
- target cultural ontology profile stack used
- treatment choice where relevant
- whether dialogue uses direct or softened imperatives
- idiom choices that are Brazilian Portuguese rather than European Portuguese
  defaults
- optional regional or cultural overlays when the source context earns them

The initial cultural ontology profile is
`data/target-language-ontology/pt-BR.yaml`.

Umbanda-related scenes, speakers, ritual contexts, or referents may add the
`data/target-language-ontology/pt-BR-umbanda.yaml` overlay. That overlay must
not activate for generic Brazilian flavor.

Regional or cultural variants should be selected through the overlay catalog at
`data/target-language-ontology/pt-BR-overlays.yaml`. Planned overlay slots are
not active profiles until reviewed examples and source context exist.

### Japanese

Japanese must be explicit about register and relationship stance. Politeness,
sentence-final force, omitted subjects, and address terms are realization
choices, not decoration.

The target lowerer should record:

- `locale: ja-JP` when a locale value is needed
- target cultural ontology profile used
- register such as `plain`, `polite`, `rough`, or `technical`
- whether subject omission preserves or harms clarity
- whether katakana, kanji, kana, or mixed script choices carry useful pressure

The initial cultural ontology profile is
`data/target-language-ontology/ja-JP.yaml`.

## Packet Shape

```yaml
schema_version: weksa.utterance_lowering.v0
source_packet_id:
target_agent_id:
agent_state_ref:
projected_context_ref:
lowering_controls:
  output_mode: dialogue
  preserve_referents:
  preserve_predications:
  allow_inference:
  max_line_count:
realizations:
  - realization_id:
    target_language:
      code:
      locale:
      script:
      register:
      cultural_ontology_refs:
    outputs:
      visible_action:
      spoken_text:
      literal_backgloss:
      natural_backtranslation:
      private_interpretation:
      intended_effect:
      trace:
```

## Output Contract

Each realization should emit:

- `spoken_text`: target-language dialogue.
- `visible_action`: optional observable behavior around the line.
- `literal_backgloss`: close structural gloss for reviewers.
- `natural_backtranslation`: natural English restatement for review only.
- `private_interpretation`: what the speaker thinks they are doing.
- `intended_effect`: what the speaker wants the line to do.
- `trace`: source fields, target-language decisions, and unresolved questions.

Backtranslations are review aids. They are not new source meaning, and later
targets must not be lowered from them.

Trace should name the cultural ontology axes that shaped the line, especially
where the target language requires a social or register choice the source packet
does not spell out.

## Speech Provider Targets

Speech providers are lowering targets for accepted realizations, not alternate
meaning sources. A provider request must consume the target-language
`spoken_text` that Weksa already accepted and must carry a trace back to the
interlingua packet, projected speaker context, and target-language profile.

MiMo V2.5 TTS is a candidate external provider target; its lowering path is
parked at tag `parked/weksa-node-body`. Its natural-language style
instruction is a projection of Weksa delivery intent, and its assistant content
is the selected target-language spoken text.

AquaSynth remains the GameCult-owned synthesis path for learned utterance
embeddings and synth-control work. MiMo and AquaSynth are sibling target
projections from Weksa-authored utterances, not owners of Weksa meaning.

## First Proof

The first proof uses the existing Nibu warning intent:

```text
do not touch the damaged ship control, because doing so may endanger you
```

The output set should include:

- English flavored by Nibu's projected context
- `pt-BR` with direct warning force and Brazilian Portuguese idiom
- Japanese with sharp, technically precise plain-register force

The proof fails if any target loses the damaged control, the player risk, or
Nibu's intervention. It also fails if the target line becomes generic assistant
politeness or random cruelty.
