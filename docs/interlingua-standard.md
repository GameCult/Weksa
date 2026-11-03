# Interlingua Standard

Version: `0.2-draft`

The Weksa interlingua is the language-neutral meaning contract between clients,
setting surfaces, authoring tools, language projects, ontology projection, and
renderers.

It is not English. It is not a grammar tree. It is not a target-language
ontology. It is the smallest structured packet that says what is meant, who is
involved, what the context is, and which facts are stable enough for a language
project to reinterpret.

## Objective

Give every Weksa client one shared way to submit meaning without forcing all
settings through one worldview.

The interlingua should support:

- game telemetry and commentary intents
- naming briefs for places, species, institutions, artifacts, and ecological
  structures
- dialogue and narration authoring
- corpus examples
- model-assisted semantic decomposition
- deterministic runtime rendering
- traceable ontology projection

## Prior Art

Weksa should learn from existing semantic annotation work without becoming a
research format clone.

- AMR gives a useful model for abstract graph meaning that is not a surface parse.
- UMR extends that direction with cross-linguistic and document-level concerns
  such as coreference, temporal relations, modal dependencies, aspect, and
  multilingual diversity.
- FrameNet keeps scenes and roles central.
- Universal Dependencies shows a useful governance pattern: common categories
  with language-specific extensions when needed.

Weksa's difference is purpose. It is a rendering contract for constructed
languages and alien ontologies, not a general-purpose annotation benchmark.

## Ownership

The interlingua owns:

- referents
- events, states, and relations
- roles between referents and predicates
- time, aspect, modality, evidence, and polarity when known
- discourse function and speaker intent
- context needed for rendering
- provenance and confidence
- extension slots with explicit namespaces

The interlingua does not own:

- target-language concept splits or collapses
- grammar obligations
- morphology
- phonology
- diachronic history
- final wording
- culture-specific metaphors unless the source meaning explicitly contains them

Those belong downstream.

## Design Principles

- Meaning before words.
- Graph before sentence.
- Explicit uncertainty beats fake precision.
- Use common fields for common meaning.
- Use namespaced extensions for client or setting pressure.
- Extensions are evidence, not authority, until a language project consumes them.
- Runtime packets must be deterministic and serializable.
- The renderer must be able to trace every target-language decision back to a
  packet field, ontology rule, or language-project rule.

## Packet Shape

The Rust types in [`crates/weksa/src/packet.rs`](../crates/weksa/src/packet.rs)
are the definition; the JSON Schemas in [`schemas/cultnet/`](../schemas/cultnet/)
are generated from them and a test fails when they drift. A packet is the
CultCache document `weksa.interlingua_packet`. This section describes the shape;
where it and the types disagree, the types win.

Two layers carry the standard. The **schema** is the Rust types: the packet
families, node ids, slots and value kinds. It changes by commit and rarely. The
**inventory** is data: the concepts, frames, roles, relation types, speech acts
and features that the schema's slots name. It is stored as
`weksa.inventory_entry` documents under appended `weksa.inventory_revision`
records, never edited in place (a changed meaning is a retire plus an add), and
a retired id still resolves for packets of earlier revisions.

Every id is an opaque form type, parsed when it is built and when a store
reads it: an inventory id is `<family>:<n>` with `n` a canonical decimal of at
least 1 (`role:1`; `role:01` is not an id), a node id is `n<k>` with `k` in
1..=9999, a packet id is `packet:<16 hex>` and an encoder id is
`encoder:<16 hex>`, an entity id is `entity:<slug>` (`[a-z0-9]` and inner
`-`, 1 to 48 bytes) and a corpus item id is `item:<slug>:<n>`. The only free
text a packet holds is the `trace` (`notes` and `rejected_readings`), which the
validator neither bounds nor reads and which is stripped before lowering: a
referent names its entity by id, and the entity store (`weksa.entity`) holds
the entity's forms per language. The packet store refuses a referent whose
entity the store lacks, a packet whose `corpus_item` the corpus lacks, and one
whose entity is neither named on that item nor its speaker or a prior-turn
speaker (a packet without a corpus item has no door in this store):
an id-set check, not a text rule. Leakage of scored text is owned outside the
validator, by the loop's context builder (what each model call sees) and the
scorer; the validator owns form and capacity. The trace is stripped before
lowering. Concept ids are opaque and carry a gloss in the inventory; concepts
are open-class and grow, while the other families are closed-class.

```text
Packet
  id, schema_version, inventory_rev, kind
  provenance   { source, corpus_item, encoder, confidence }
  context      { setting, medium, participants, audience }
  discourse    { speech_act, addressee, features }
  referents    [ { id, kind, concept, entity, features } ]
  predications [ { id, concept, frame, roles: [ { role, filler } ], features } ]
  relations    [ { relation, from, to, features } ]
  constraints  { preserve, deterministic, max_latency_ms }
  extensions   [ { key: InvId (family extension), value: Grade | Option | Flag } ]
  trace        { notes, rejected_readings }
  stance       { features }
```

`schema_version` and `inventory_rev` name the exact schema and inventory
revision a packet was written against. A grade is an integer permille,
`0..=1000`, and the published schema says so. There are no floats.

### Validation

A packet exists only if the validator's rules pass and the packet store admits
it. `validate` judges and returns `Ok(())` or the refusals; the packet store's
`admit` is the one door and the only source of the lowering view. A refusal
names the rule.

- V1: `schema_version` is the version these types define.
- V2: at least one of `referents`, `predications`, `relations` is present.
- V3: a node id appears once across referents and predications.
- V4: every role filler, relation end, addressee, participant, audience member
  and preserved node names a node.
- V5: every inventory id resolves at the packet's `inventory_rev`: introduced
  at or before it, and not retired at or before it.
- V6: an inventory id is of the family its slot takes.
- V7: a feature or extension value fits its entry's value kind (a grade is a
  grade, an option is one the entry declares, a flag is a flag).
- V8: a feature sits only in a slot its entry declares.
- V10: `inventory_rev` has a revision record.
- V11: a feature id appears at most once in one slot's feature list, an
  extension id at most once per packet, a node id at most once in each of
  `participants`, `audience` and `preserve`, and a role id at most once in one
  predication's roles.
- V12: a packet has at most 64 referents, predications and relations together
  (a calibration constant, capacity and not meaning).

V9 is retired and not renumbered: the form of a key or a grade is its type, and
an extension key's family is V6. `PacketStore::admit` returns the lowering view of a
packet it admits; the view carries the meaning and the stance, no packet id, no
provenance and no trace, and is built nowhere else. The store also refuses a
packet that names a withdrawn corpus item (packets admitted before the
withdrawal stay stored). A packet is admitted against the inventory the packet store
reads at the packet's revision, never one the caller supplies.

### `kind`

`Utterance`, `NameRequest`, `CommentaryIntent`, `Narration`, `DialogueLine`,
`CorpusExample`, `DebugProbe`.

### `provenance`

`source` is `Authored`, `ModelDecomposition`, `GameTelemetry` or
`ImportedCorpus`. `corpus_item` is the required `item:<slug>:<n>` key of the
corpus item the packet was minted for, `encoder` names what wrote it, and `confidence` is a grade.

### `context`

`setting` and `medium` are concept ids. `participants` and `audience` are node
ids.

### `discourse`

Why the packet is said: `speech_act` is a speech-act id, `addressee` a node id,
and `features` carry source-side nuance on content (hedge, certainty, irony and
whatever the inventory grows). This is source-side communicative pressure, not
target-language register.

### `stance`

The speaker's felt affect and how much of it the speaker holds back. `features`
is a feature list in slot `Stance`, checked like every other (V5 to V8, V11).
The seed carries felt affect as grades of how strongly it is felt (anger, fear, sadness,
joy, disgust, surprise, contempt, affection, shame) and `restraint`, a grade for
how much of the felt affect is suppressed (0 none, 1000 fully). The stance is
what is felt, never what is displayed: how a feeling shows, and how much of a
restrained one leaks, are cultural, and each language's ontology maps felt
affect and restraint to its own register. A held-back feeling is not a claim
about the world, so it is never a predication. An empty stance is valid. The
stance reaches the lowerer in the lowering view.

### `referents`

Entities, groups, places, abstractions and the rest. `kind` is a referent-kind
id (the inventory seeds `person`, `group`, `organism`, `place`, `artifact`,
`institution`, `ecology`, `route`, `memory`, `event`, `abstract`), `concept` is
a concept id, and `entity` is the id of the named entity when the referent has one.

### `predications`

Events, states, processes and scenes. `concept` and `frame` are ids; `roles`
pair a role id with a filler node (the inventory seeds `agent`, `patient`,
`experiencer`, `instrument`, `source`, `goal`, `location`, `beneficiary`). Tense,
aspect, polarity, modality and evidentiality are features with declared option
sets, not fields.

### `relations`

Edges not owned by one predication. `relation` is a relation id (the inventory
seeds `coreference`, `part_of`, `member_of`, `located_at`, `near`, `before`,
`after`, `causes`, `enables`, `prevents`, `owns`, `owes`, `knows`, `names`,
`represents`, `contrasts_with`), and `from` and `to` are node ids.

### `constraints`

`preserve` lists nodes the lowering must keep, `deterministic` and
`max_latency_ms` bound the renderer. Requirements about which outputs a
renderer must or must not produce belong to the target profile, not the packet.

### `extensions`

Setting or client data. An extension key is an inventory entry of family
`extension` (`extension:<n>`) whose entry declares the value kind, so a key is
an id and not a string. Extensions must not replace common fields when common
fields suffice, a language project may ignore them, and one that becomes
broadly required is promoted into the inventory or the schema.

### `trace`

Authoring and debug notes. The trace is stripped before lowering: the lowering
input type has no trace field.

## Minimal Examples

These examples illustrate meaning in the 0.1 shape, with free-text labels and
predicates. Those fields no longer exist; the examples are not valid packets.

### Name Request

```yaml
interlingua_version: 0.1-draft
packet_id: zyphos-name-001
kind: name_request
provenance:
  source_type: authored
context:
  setting: Eusocial Interbeing
referents:
  - id: target
    kind: ecology
    label: disconnected tree network
  - id: namer
    kind: group
    label: Airawa imperial surveyor
relations:
  - type: names
    from: namer
    to: target
extensions:
  zyphos.memory_relation: severed_from_saturated_substrate
  zyphos.boundary_permeability: hidden_refusal
```

### Commentary Intent

```yaml
interlingua_version: 0.1-draft
packet_id: rtd-commentary-001
kind: commentary_intent
context:
  setting: Rust to Dust
  medium: broadcast
discourse:
  speech_act: commentary
  audience_function: spectacle
referents:
  - id: actor
    kind: person
  - id: target
    kind: person
predications:
  - id: event
    predicate: eliminate
    frame: combat
    roles:
      agent: actor
      patient: target
extensions:
  rtd.aesthetic_reading:
    form: raw
    spectacle_value: high
```

## Promotion Rule

The interlingua should stay boring. When a new requirement appears:

1. Put it in a namespaced extension.
2. Use it in at least two real packets.
3. Ask which invariant it protects.
4. Promote it only if multiple clients or language projects need it.

No generic bucket earns permanence by sounding useful. That is how a schema
becomes a junk drawer with a hat.

## References

- [Abstract Meaning Representation 1.0 Specification](https://www.isi.edu/results/publications/14628/abstract-meaning-representation-amr-1-0-specification/)
- [Uniform Meaning Representation release paper](https://aclanthology.org/2024.lrec-main.229.pdf)
- [Global FrameNet](https://www.globalframenet.org/)
- [Universal Dependencies Guidelines](https://universaldependencies.org/guidelines)

