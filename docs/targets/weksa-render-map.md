# Map: Weksa render (dictionary, GF grammar, no model)

Map for the target [weksa-render.md](weksa-render.md). Imagination,
2026-11-20, session `weksa-self-20261119`. This page has no cut specs. The
operator sees it before anything is deleted, and Self opens the cuts after
that. Questions cited here are in the mind under `weksa-lowering:question:`.

## Pre-implementation statement

- **Objective.** Render interlingua packets into constructed languages
  deterministically, by dictionary and GF grammar. Prove the machinery on
  English and Japanese first.
- **Current mechanism.** A validated packet reaches a model as a lowering
  prompt (`context.rs` `Prompt::lower`). The prompt carries the language
  ontology's free-text register entries, chosen from stance and scene. The
  model's JSON answer (`encode.rs` `LoweringDraft`) is the surface text. A golf
  loop (`run.rs`) scores that text with chrF and meaning cosine and grows the
  inventory. No deterministic renderer exists on main.
- **Invariants.** The labelled invariants of the target: `one-machinery`,
  `no-model-renders`, `abstract-level-choices`, `round-trip`,
  `render-deterministic`, `render-is-pure`, `dictionary-append-only`,
  `coinage-deterministic`, `learnable`, `packet-owns-meaning`, `typed-packet`,
  `render-traced`.
- **Intended change.** The planner owns the tree and a concrete grammar owns
  the surface. The dictionary owns words, each language definition owns its
  sounds and history, and the validator still owns whether a packet exists.
  Four owners lose their standing. The model is no longer an owner. The
  inventory loses both its families: open-class concepts go to dictionary keys
  and closed classes become packet-schema enums. The language ontology is
  replaced by abstract-level choices plus concrete realization. The corpus
  stops owning packets.
- **Cut line.** The golf loop, the scorer, the model port, prompt building,
  the inventory, the language ontology and the split all go. So do the
  Validation and Test corpora built for the loop (`data/original`), the
  model-lowering docs, examples and prompts, and the speech-handoff tools.
- **Subtraction budget.** About 22,000 of 40,000 text lines under the five
  audited roots retire outright (table below). Of the 11,828 lines in files
  that stay but shrink, roughly half go: `store.rs` loses three of its five
  stores, `main.rs` loses `loop`, `score`, `split`, `inventory` and `ontology`,
  and the test files lose matching shares. New surface is net additive in four
  places: the FFI wrapper, the planner, the dictionary store with coinage, and
  the grammars. Each buys a capability the ruling asked for. The largest is the
  conlang grammar generator, if question `conlang-grammar-authoring` rules
  `generated`. No existing owner can serve as grammar or dictionary: GF
  replaces a grammar Weksa never had, and the inventory's revision machinery
  goes rather than turning into a dictionary.

## Identity, lifecycle and authority

| Kind | What names it | How it changes over time | Who decides |
|---|---|---|---|
| Packet | Key `packet:<digest>` of the packet's canonical encoding: content-addressed, so the same meaning written twice is one packet. The hash is whatever follow-up `collision-resistant-digest` settles, since FNV-1a 64 is not enough for content addressing. CultCache type `weksa.interlingua_packet`, `schema_version` 6. Concept slots carry `ConceptId` (form per question `concept-identity`); frame, role, relation, speech act, feature and referent kind are closed enums; referents cite `EntityId`; provenance names its author (Operator, setting owner, Model-drafted with its reviewer, Persona), not a corpus item | Immutable. A changed meaning is a new packet. A schema change is a commit that bumps `schema_version` and regenerates the published JSON Schema; an old-version packet is re-authored, never migrated in place | The author writes it. `PacketStore::admit` admits it only through `validate`, which checks form, structure, bounds, consent, entity ids against the entity store, and concept ids against the dictionary's concept set. Nothing else writes packets |
| Abstract tree | Derived and never stored. Named in a trace by the packet id, the planner's commit and the PGF digest; printed in GF syntax | Recomputed on every render; changes when the planner or the abstract changes. A calibration fixture may pin an expected tree as test data (a golden value, not an authority) | The planner in `crates/weksa`, from the packet view (packet minus trace), the scene relation `LoweringInput::with_scene` reads today, the speaker's stable axes, and entity forms. Its rules change by commit and are language-neutral; a written default fills any distinction the packet is silent on, and the trace names it |
| GF abstract syntax | Module `Weksa` (`grammars/Weksa.gf` if `gf-grammar-home` rules `this-repo`), plus its lexical module generated from the dictionary's concept set. Identified by commit and by the PGF digest | Changes by commit, in the same commit as the planner code that emits a new constructor. A constructor enters when any language needs the distinction. Removing one is a breaking change: the planner and every concrete move with it | A cut (Hands under a spec), language-neutral; Soul checks that no constructor is one language's private path (`one-machinery`) |
| GF concrete, natural language | `WeksaEng`, `WeksaJpn`: a functor over the RGL `Syntax` interface plus a lexicon instance generated from the dictionary, as the probe built it (`gf-probe.functor`). RGL pinned by release (`gf-rgl-20260403`) | By commit; an RGL upgrade is its own commit with a calibration re-read | Weksa writes the application concrete (`gf-adopted`). The RGL upstream owns the resource grammar. Japanese keigo beyond Plain/Resp is Weksa's to write. A native reader attests calibration (question `calibration-evidence`) |
| GF concrete, conlang | `Weksa<Lang>` (e.g. the imperial language's slug) | Generated from the language definition at each definition revision and never hand-edited, if `conlang-grammar-authoring` rules `generated`; hand-written by commit if `hand-written` | The language's owner through its definition: the operator for Rust to Dust; Deru holds Zyphos canon and Weksa drafts within it |
| Compiled PGF | `Weksa.pgf`, named by content digest | Derived build artifact: built by the GF compiler in the Ubuntu 24.04 container on Yggdrasil, not committed. Rebuilt when any grammar or dictionary input changes | The build. The digest in every trace says which build rendered a text |
| Dictionary entry | Key `dict:<language>:<concept>:<n>` (`n` from 1 per pair); type `weksa.dictionary_entry`. Fields: language, concept, GF category (`N`, `V2`, `A`, ...), paradigm and citation forms (orthographic for natural languages, phonemic roots for conlangs), provenance (`Imported{source, version}`, `Coined{...}`, `Authored{by}`), and the entry it supersedes. Proper names are not entries: they stay in the entity store, whose per-language forms generate the `PN` lexicon | Append-only. Never edited or deleted; a correction is entry `n+1` superseding `n`. Render uses the newest entry of each pair and names it in the trace. Concepts appear by being named in an entry; a concept never disappears | One writer, `DictionaryStore::admit`, for every source: import (gf-wordnet or DictEng for English and Japanese, pinned by commit, subject to question `concept-identity` and a licence check), `weksa coin` for conlangs, and hand authoring by the language's owner |
| Coined word | A dictionary entry whose provenance is `Coined{definition_rev, attempt}`; its root is phonemic | Never re-coined. A later definition revision does not change an admitted coinage; only an `Authored` entry superseding it can. Daughter-stage forms are derived by the definition's sound-change stages, not stored (idea `render-map.store-daughter-forms`, Recorded) | The coinage function: a per-(language, concept) seed over the definition's root templates and phonotactics, with a deterministic re-draw (`attempt` 1, 2, ...) on collision with an existing form. The language owner may veto by superseding |
| Phonology rules | Part of the language definition: phone inventory (today `data/phonology/<lang>.phones.tsv`), phonotactic constraints, syllable template, stress. For a conlang, the constraints the operator or setting owner gave (e.g. Deru's for Sa'ueia and Airawa) | Revised with the definition. A revision that would make an admitted root illegal is refused unless it adds a sound-change stage that accounts for the root | The language owner; for natural languages the operator rules a reference variety (precedent: the en-US inventory and Nibu's pronunciation ruling) |
| Sound-change rules | Ordered stages `stage:<language>:<k>` in the language definition, each an ordered list of rewrite rules over phones | Append-only by stage. A stage that any kept text cites is frozen; new history appends a stage | The language owner. The engine applies stages in order and records nothing else |
| Language definition | `lang:<slug>@<rev>`, a typed document set: phonology, orthography (phonemic to script, token joining), morphology paradigm classes, grammar parameters (if generated), coinage templates, sound-change stages, and the abstract categories the language marks. English and Japanese have definitions too, thin ones: orthography, phone inventory, the RGL concrete pointer | Appended revisions; every render names its revision. `state/map.yaml`'s "language project" maps here | Owner per language, as above. A definition is data checked by the engine (legal phones, paradigm completeness), never a model's output admitted unread |
| Entity (kept) | `entity:<slug>`, type `weksa.entity`: per-language forms, citation form first, plus ruled pronunciation | Forms only grow; a pronunciation is added once and frozen (today's door) | Whoever mints the named thing: the operator, a setting owner, or an importer |
| Speaker, scene (kept) | `speaker:<slug>@<n>`, `scene:<group>@<n>`, with `inventory_rev` dropped and feature values as schema enums | Append-only revisions (today's door) | The annotator each names (`Annotator`); follow-up `speaker-scene-operator-annotator` still owes the operator's attestation shape |
| Calibration reference | `ref:<source-slug>:<n>`, type shrunk from `weksa.corpus_item`: text, language, public-domain provenance, and the packet it calibrates | Immutable; withdrawn, never deleted | The importer, under the Gutenberg terms (`docs/research/2026-11-13-gutenberg-terms.md`) and `corpus-labels-tell-truth` |
| Render (text and trace) | Not stored by the renderer. A calibration run that keeps renders for a reader names each by packet id, definition revision and PGF digest | Recomputed; a stored copy is evidence, not authority | Derived from the tree by the concrete and the orthography rule |

## Authority map

- **Owner.** Each decision has one owner:
  - The validator decides whether a packet exists.
  - The planner decides which abstract tree a packet becomes.
  - A language's concrete grammar decides that tree's token string.
  - The language definition decides orthography, phonology, coinage and
    sound change.
  - The dictionary decides which word stands for a concept in a language.
- **Inputs.**
  - The planner reads the packet view, the scene relation, speaker axes and
    entity ids.
  - The concrete reads only the tree.
  - Orthography reads tokens and the definition.
  - Coinage reads the definition, the concept id and the dictionary's
    existing forms.
- **Outputs.** Text plus trace, or a typed refusal. Refusals include an
  invalid packet, a missing dictionary entry, a tree the abstract refuses, and
  a round-trip failure.
- **Derived state.**
  - Cache-only: the abstract tree, the PGF, the generated GF lexicon modules
    and (if ruled) generated conlang concretes.
  - Derived per render: daughter-stage forms, orthographic text, phones.
  - Test data, not authority: calibration renders.
- **Demotions.**
  - The model lowerer is no longer an owner. Surface text is derived from the
    abstract tree by the concrete grammar.
  - The language ontology (`onto:<language>:<n>`) is no longer an owner. The
    register choice is derived by the planner from stance and scene into the
    tree, and its realization belongs to each concrete.
  - The inventory is no longer an owner. Concepts are dictionary keys, and
    closed classes are packet-schema enums changed by commit.
  - `inventory_rev` is dead. An append-only dictionary means a concept id,
    once valid, stays valid, so resolution needs no revision pin.
  - The corpus no longer owns packets. A calibration reference points at a
    packet, never the reverse, so `provenance.corpus_item` is dead. That also
    retires follow-up `live-packet-door`'s premise.
  - Scores are dead. Calibration evidence is the round trip plus human
    reading (question `calibration-evidence`).
  - `formality`, `warmth` and `intensity` stay out of the packet, as
    `stance-in-packet-speaker-scene-documents` ruled.
- **Forbidden writers.**
  - Any model call on the render path.
  - `Prompt::lower`, `LoweringDraft`, ontology entries, inventory revisions
    and edit drafts, and every `run` cycle.
  - Render writing the dictionary.
  - A hand edit of generated GF.
  - A stored daughter form.
  - A per-language branch in the planner.
- **Shared paths.**
  - Every caller renders through one `render(packet, language)`: plan,
    check, linearize, round trip, orthography, trace. Callers include the CLI
    (`weksa render`), tests, calibration and any later consumer.
  - Every dictionary write goes through `DictionaryStore::admit`: import,
    coin and authored entries alike.
  - Every packet goes through `PacketStore::admit`.
- **Deletion line.**
  - The loop and everything only it reads are deleted before the GF wrapper
    lands. That covers `run`, `score`, `model`, `context`, `encode`, `split`,
    `ontology`, `data/original` and their tests, docs, examples and prompts.
  - Packet schema v6 (no inventory, no corpus item) lands before the planner,
    so no planner is ever written against inventory ids.

## Cut order (a sketch for Self, not specs)

1. `retire-loop`: one deletion cut over everything marked retires in
   `crates/weksa`, `data/`, `docs/`, `tools/` and outside, plus the CLI
   subcommands. Proof: the remaining tests pass, `cargo tree` has no `ureq`,
   and a grep finds no `Prompt`, `ModelPort` or `RunStores`.
2. `packet-v6`: the inventory goes; closed enums; `ConceptId` (waits on
   `concept-identity`); no `inventory_rev`, no corpus item; speaker and scene
   enums; `CorpusStore` becomes calibration references; the dictionary store
   with its concept set; Dao packets re-authored.
3. `gf-wrapper`: one Rust module over `libpgf`, with checked expressions and
   owned strings, built and tested in the Yggdrasil container; a round trip on
   a fixture PGF.
4. `render-en`: the `Weksa` abstract, `WeksaEng` and the planner on the Dao
   slice; `weksa render`; the round-trip gate; the trace.
5. `render-ja`: `WeksaJpn`, register as abstract nodes, orthographic token
   joining, and the first calibration reading.
6. `conlang-definition` and `coin`: the imperial language definition, the
   coinage function and its concrete (shape per `conlang-grammar-authoring`;
   timing per `conlang-start`).
7. `diachrony`: sound-change stages and derived forms.

Afterwards, Life rewrites `state/map.yaml` from the target. Its June
constraints (no CLI, no crate) are stale, though its renderer invariants
survive.

## Retirement map

Every tracked file under the five audited roots, plus everything outside them
by directory. Verdicts:

- **keeps**: the file stays as an owner in the new pipeline.
- **keeps, shrinks**: the file stays and loses the parts named.
- **moves**: the surviving content goes to a new owner or shape.
- **retires**: deleted, with git history keeping it.
- **unknown**: needs the operator.

Counts are newline counts at main 1e06f00. CultCache stores, PDFs and images
are binary and counted in bytes.

### Surviving owners and what each owns in the new pipeline

- `packet.rs`, `validate.rs`, `PacketStore`: the packet and whether it
  exists. `LoweringInput` becomes the planner's input view.
- `entity.rs`, `EntityStore`, `data/entity/entity.cc`: named things and their
  per-language forms, which become each language's `PN` lexicon; ruled
  pronunciation.
- `speaker.rs`, `SpeakerStateStore`, `tests/scene.rs`: the speaker and scene
  inputs from which the planner picks abstract register and relation nodes.
- `phones.rs`, `lower.rs` (`pronounce`), `data/phonology/`: the phone
  inventory and segmenter, the seed of each language definition's phonology.
  `weksa pronounce` reads ruled IPA into phones.
- `form.rs`, `schema.rs`: form newtypes; published JSON Schemas.
- `corpus.rs`, `data/daodejing/`: calibration references and the calibration
  bed's packets.

### `crates/weksa/`

| Path | Lines | Verdict | New owner or reason |
|---|---:|---|---|
| `src/entity.rs` | 84 | keeps | Entity identity and per-language forms: the source of each language's proper-name (`PN`) lexicon; ruled pronunciation. |
| `src/form.rs` | 301 | keeps | Form newtypes and the refused-format-character rule. |
| `src/phones.rs` | 152 | keeps | Phone inventory and segmenter: the seed of the phonology owner, per language. |
| `src/schema.rs` | 34 | keeps | JSON Schema publication. |
| `tests/display.rs` | 121 | keeps | Error text names the rule, never the caller's ids. |
| `tests/entity_door.rs` | 642 | keeps |  |
| `tests/format_refused.txt` | 171 | keeps | Oracle for `form.rs` and `entity_door.rs`. |
| `tests/lower.rs` | 306 | keeps | `pronounce` and `phones`. |
| `tests/mutants_a.rs` | 25 | keeps | Calendar date pins. |
| `tests/speaker_state.rs` | 503 | keeps |  |
| `Cargo.toml` | 28 | keeps, shrinks | `ureq` goes with `score/embed.rs`; links the GF C runtime (`libpgf`, `libgu`) behind the one wrapper module. |
| `src/lib.rs` | 49 | keeps, shrinks | Re-export list shrinks to the surviving owners. |
| `src/lower.rs` | 178 | keeps, shrinks | Entity port, `pronounce`, `phones`. Renamed `pronounce.rs`; `TARGET_LANGUAGE` goes. |
| `src/main.rs` | 1,461 | keeps, shrinks | CLI. `loop`, `score`, `split`, `inventory`, `ontology`, `corpus import` go; `validate`, `entity`, `scene`, `packet`, `pronounce`, `schema` stay; `render`, `parse`, `coin` arrive later. |
| `src/packet.rs` | 436 | keeps, shrinks | Packet owner, schema v6: `inventory_rev` and `provenance.corpus_item` go; concept slots become `ConceptId` (dictionary key); frame, role, relation, speech act, feature and referent kind become closed enums of the schema. |
| `src/speaker.rs` | 134 | keeps, shrinks | Speaker and scene documents: the inputs from which the planner chooses abstract register and relation nodes. Feature `InvId`s become schema enums; `inventory_rev` goes. |
| `src/store.rs` | 1,362 | keeps, shrinks | `PacketStore` (admits without a corpus item), `EntityStore`, `SpeakerStateStore` stay; `CorpusStore` shrinks to calibration references; `InventoryStore` and `OntologyStore` go; `DictionaryStore` arrives beside them. |
| `src/validate.rs` | 818 | keeps, shrinks | Owns whether a packet exists. Inventory-at-revision rules (v5, v6, v8, v10) become unrepresentable in the enums; gains "every concept is in the dictionary". `LoweringInput` becomes the planner's input. |
| `tests/cli.rs` | 2,274 | keeps, shrinks | About 14 of 36 tests are loop, corpus or score and go; validate, entity, schema, pronounce and echo-hygiene tests stay. |
| `tests/common/mod.rs` | 445 | keeps, shrinks | Fixtures shrink with the stores. |
| `tests/entities.rs` | 597 | keeps, shrinks | Corpus name-span tests go. |
| `tests/forms.rs` | 169 | keeps, shrinks | `inv_id_canonical`, `options_bounded` go. |
| `tests/mutants_b.rs` | 297 | keeps, shrinks | Ontology and relower pins go. |
| `tests/scene.rs` | 463 | keeps, shrinks | The scene relation the planner reads; lowering-prompt assertions go. |
| `tests/seeds.rs` | 38 | keeps, shrinks | Replays only surviving seed stores. |
| `tests/stores.rs` | 1,893 | keeps, shrinks | Corpus S1-S6 and inventory revision tests go; packet and entity admission stay. |
| `tests/validate.rs` | 811 | keeps, shrinks | Inventory-resolution rules go; structure, consent, bounds stay. |
| `src/corpus.rs` | 179 | moves | Shrinks to the calibration reference (text, language, public-domain provenance, the packet it calibrates). Split group, speaker, prior turns, name spans and `ModelWritten` go. |
| `src/daodejing.rs` | 328 | moves | Its hand-authored packets are re-authored under schema v6 as the calibration bed; its inventory revision 3 goes. |
| `tests/daodejing.rs` | 56 | moves | Becomes the calibration-bed admission test. |
| `tests/split_schema.rs` | 189 | moves | `published_schemas_match` moves to a schema test; the split tests go. |
| `src/context.rs` | 1,028 | retires | Prompt builder for model encode, lower and mint. |
| `src/context/mutants.rs` | 14 | retires |  |
| `src/encode.rs` | 1,089 | retires | Model-answer JSON doors (`PacketDraft`, `EditDraft`, `OntologyDraft`, `LoweringDraft`) and the name-span work importer. A non-Rust packet author uses the packet's own serde through `validate`, not a draft type. |
| `src/encode/mutants.rs` | 13 | retires |  |
| `src/inventory.rs` | 610 | retires | Concepts go to the dictionary; the closed families of the seed become packet-schema enums (about 160 lines of seed content carried as enums, not as revisions). |
| `src/model.rs` | 130 | retires | The loop's model port. |
| `src/ontology.rs` | 207 | retires | Model-prompt register entries. Register is chosen by the planner at abstract level and realized by each concrete grammar. |
| `src/run.rs` | 4,112 | retires | Golf-loop run, rounds, cycles, accept rule, relower, ablate, calibrate. |
| `src/run/mutants.rs` | 21 | retires |  |
| `src/score/chrf.rs` | 60 | retires | chrF. |
| `src/score/diagnostics.rs` | 167 | retires | Leak diagnostics. |
| `src/score/embed.rs` | 113 | retires | Meaning cosine via Ollama. |
| `src/score/mod.rs` | 363 | retires | Scorer. |
| `src/score/words.rs` | 96 | retires | Scorer word reader. |
| `src/split.rs` | 223 | retires | Train/Validation/Test split and Checkpoint. |
| `src/split/mutants.rs` | 11 | retires |  |
| `tests/checkpoint_door.rs` | 107 | retires | Split checkpoint tripwire. |
| `tests/context.rs` | 1,225 | retires |  |
| `tests/golf_loop.rs` | 6,560 | retires |  |
| `tests/modifier_letters.txt` | 411 | retires | Oracle for the corpus S6 duplicate-text fold. |
| `tests/mutants_c.rs` | 136 | retires | Pins for context, encode, run. |
| `tests/ontology_mint.rs` | 402 | retires |  |
| `tests/original_corpus.rs` | 1,384 | retires |  |
| `tests/score.rs` | 876 | retires |  |

### `data/`

| Path | Lines | Verdict | New owner or reason |
|---|---:|---|---|
| `entity/entity.cc` | 607 B | keeps | Entity store. |
| `phonology/en-US.phones.tsv` | 50 | keeps | en-US phone inventory. |
| `phonology/fixtures/nibu-en-US.phones.json` | 1 | keeps |  |
| `daodejing/corpus.cc` | 2,959 B | moves | Legge items become calibration references. |
| `daodejing/entity.cc` | 319 B | moves | Calibration bed. |
| `daodejing/packets.cc` | 2,656 B | moves | Re-authored under schema v6. |
| `corpus/corpus.cc` | 975 B | retires | The proof packet's corpus item; packets no longer cite one. |
| `corpus/speaker-state.cc` | 1 B | retires | Empty store. |
| `daodejing/inventory.cc` | 11,369 B | retires |  |
| `daodejing/speaker-state.cc` | 1 B | retires | Empty store. |
| `interlingua/inventory.cc` | 8,777 B | retires | Concept inventory; closed classes move to the schema. |
| `ontology/ontology.cc` | 12,438 B | retires | Language ontology entries. |
| `original/corpus.cc` | 45,018 B | retires | Validation and Test corpora built for the loop. |
| `original/entity.cc` | 2,600 B | retires | Its cast, minted for those corpora. |
| `original/inventory.cc` | 35,233 B | retires | Model-drafted concepts for those corpora. |
| `original/speaker-state.cc` | 60,158 B | retires | Keeper scenes for those corpora. |
| `roots/README.md` | 7 | retires | Placeholder; the dictionary and language definitions get real homes. |
| `typology/README.md` | 7 | retires | Placeholder. |

### `docs/`

| Path | Lines | Verdict | New owner or reason |
|---|---:|---|---|
| `project-model.md` | 69 | keeps | Engine, language project, setting surface: still the layering. |
| `research/2026-10-29-interlingua-lowering-prior-art.md` | 90 | keeps | Dated evidence (GF, KANT, Abstract Wikipedia). |
| `research/2026-11-13-gutenberg-terms.md` | 59 | keeps | Calibration references come from Gutenberg editions. |
| `runtime-authoring-split.md` | 61 | keeps | Deterministic hot path, models only in authoring: matches the rulings. |
| `interlingua-standard.md` | 375 | keeps, shrinks | Rewritten for schema v6: concept ids and closed enums in place of inventory ids. |
| `best-modern-approach.md` | 352 | moves | Root generation, morphology and diachrony doctrine feed the language-definition section; the model-assisted sections go. |
| `english-pronunciation-lowering.md` | 141 | moves | Ruled entity pronunciation moves under the phonology owner; flavored-English planning goes. |
| `prior-art-and-plan.md` | 195 | moves | The survey (UD Polite, CoCoA-MT, KeiCO) moves to `docs/research/` as evidence for Japanese register; the LLM plan goes. |
| `targets/dao-de-jing-test-bed.md` | 119 | moves | Becomes the calibration-bed section; chrF numbers go. |
| `architecture.md` | 62 | retires | Superseded by the target. |
| `calibration.md` | 120 | retires | Loop accept-rule calibration. |
| `flavored-english-lowering.md` | 186 | retires | Model-lowering target. |
| `language-model.md` | 33 | retires | Superseded. |
| `multilingual-utterance-lowering.md` | 202 | retires | Model-lowering target. |
| `name-span-annotation.md` | 52 | retires | Corpus name spans. |
| `research/2026-10-29-golf-loop-prior-art.md` | 88 | retires | Evidence for a retired machine; git history keeps it. |
| `research/2026-10-29-lowering-strategy-prior-art.md` | 71 | retires | Fine-tuning and embedding mapping. |
| `research/2026-10-29-weksa-substrate-facts.md` | 69 | retires | Facts about the parked Node body. |
| `roadmap.md` | 49 | retires | Superseded by the target. |
| `scene-annotation.md` | 116 | retires | Keeper scenes of `data/original`. |
| `targets/interlingua-lowering-map.md` | 660 | retires | Superseded by `weksa-render-map.md`. |
| `targets/interlingua-lowering.md` | 94 | retires | Superseded by `weksa-render.md`. |
| `utterance-embedding.md` | 256 | retires | AquaSynth handoff; not a consumer. |
| `verse-service-contract.md` | 31 | retires | Live service; not a consumer. |
| `cultural-ontology-meta-analysis.tex` | 1,505 | unknown | Sociolinguistic research on register; may inform which relation categories the abstract carries. The operator's research artifact. |
| `research/cultural-ontology/sources/manifest.md` | 121 | unknown | Goes with the paper above. |

### `tools/`

| Path | Lines | Verdict | New owner or reason |
|---|---:|---|---|
| `proposals-page/.gitignore` | 1 | keeps | weksa-body surface for setting owners; not part of the renderer. |
| `proposals-page/README.md` | 9 | keeps |  |
| `proposals-page/generate.py` | 63 | keeps |  |
| `proposals-page/proposals.json` | 151 | keeps |  |
| `emit_tiny_panphon_handoffs.py` | 381 | retires | AquaSynth speech-training handoff; not a consumer. |
| `validate_tiny_panphon_handoffs.py` | 230 | retires | Same. |

### `state/`

| Path | Lines | Verdict | New owner or reason |
|---|---:|---|---|
| `scratch.md` | 23 | keeps | Disposable by design. |
| `map.yaml` | 295 | moves | Stale since June; rewritten from `weksa-render.md` by Life. |

### Outside the five roots

| Path | Files | Lines | Bytes (binary) | Verdict | New owner or reason |
|---|---:|---:|---:|---|---|
| `.gitignore` | 1 | 24 | 0 | keeps |  |
| `.voidbot/` | 6 | 595 | 80,200 | keeps | Weksa's Persona face; weksa-body, not this target. |
| `AGENTS.md` | 1 | 63 | 0 | keeps |  |
| `CLAUDE.md` | 1 | 3 | 0 | keeps |  |
| `Cargo.lock` | 1 | 1,009 | 0 | keeps | Shrinks as `ureq` and its tree leave with the Ollama embedder. |
| `Cargo.toml` | 1 | 3 | 0 | keeps | Workspace manifest. |
| `schemas/cultnet/weksa.entity.schema.json` | 1 | 171 | 0 | keeps |  |
| `schemas/cultnet/weksa.interlingua_packet.schema.json` | 1 | 502 | 0 | keeps | Regenerated at schema v6. |
| `schemas/cultnet/weksa.scene.schema.json` | 1 | 225 | 0 | keeps | Regenerated without `inventory_rev`. |
| `schemas/cultnet/weksa.speaker.schema.json` | 1 | 171 | 0 | keeps | Regenerated without `inventory_rev`. |
| `schemas/project-state.md` | 1 | 37 | 0 | keeps |  |
| `settings/` | 13 | 1,923 | 0 | keeps | Setting pressure and language-design briefs: inputs to language definitions, owned by setting owners. |
| `README.md` | 1 | 118 | 0 | keeps, shrinks | Rewritten from the target. |
| `schemas/README.md` | 1 | 44 | 0 | keeps, shrinks | Its "no executable schema yet" premise is stale; rewritten. |
| `schemas/interlingua-packet.md` | 1 | 64 | 0 | keeps, shrinks | Rewritten for schema v6. |
| `examples/first-contact/` | 1 | 13 | 0 | moves | Its placeholder slice (one ontology slice, one grammar, one phonology, one morphology pattern, one sound change, a few packets, glossed output) is the first conlang slice's acceptance list. |
| `schemas/cultnet/weksa.corpus_item.schema.json` | 1 | 318 | 0 | moves | Becomes the calibration reference schema. |
| `schemas/language-project-state.md` | 1 | 24 | 0 | moves | Seed of the language-definition contract. |
| `settings/eusocial-interbeing/sa-ueia/` | 4 | 515 | 0 | moves | The phonotactic checker and morpheme table become the first Zyphos language definition's phonology data and rules; until then they stay. |
| `examples/cultcache-lowering/` | 10 | 438 | 0 | retires | Model-lowering fixtures and critiques. |
| `examples/english-lowering/` | 7 | 494 | 0 | retires | Model-lowering fixtures. |
| `examples/multilingual-lowering/` | 3 | 213 | 0 | retires | Model-lowering fixtures. |
| `examples/persona-lowering/` | 4 | 118 | 0 | retires | Model-lowering fixtures. |
| `examples/speech-training/` | 10 | 2,675 | 0 | retires | AquaSynth handoff fixtures. |
| `prompts/` | 2 | 151 | 0 | retires | Model-lowering prompts. |
| `schemas/cultnet/weksa.inventory_entry.schema.json` | 1 | 119 | 0 | retires |  |
| `schemas/cultnet/weksa.inventory_revision.schema.json` | 1 | 69 | 0 | retires |  |
| `schemas/cultnet/weksa.loop_decision.schema.json` | 1 | 230 | 0 | retires |  |
| `schemas/cultnet/weksa.loop_round.schema.json` | 1 | 369 | 0 | retires |  |
| `schemas/cultnet/weksa.loop_run.schema.json` | 1 | 136 | 0 | retires |  |
| `schemas/cultnet/weksa.ontology_entry.schema.json` | 1 | 277 | 0 | retires |  |
| `schemas/cultural-ontology-profile.md` | 1 | 275 | 0 | retires |  |
| `schemas/english-lowering-packet.md` | 1 | 64 | 0 | retires |  |
| `schemas/english-pronunciation-plan.md` | 1 | 37 | 0 | retires |  |
| `schemas/utterance-embedding-handoff.md` | 1 | 77 | 0 | retires |  |
| `schemas/utterance-lowering-packet.md` | 1 | 99 | 0 | retires |  |
| `scripts/scene-annotation/` | 3 | 151 | 0 | retires | Keeper-scene annotation for `data/original`. |
| `docs/research/cultural-ontology/sources/pdfs/` | 17 | 0 | 100,502,402 | unknown | Cached source PDFs for the cultural-ontology paper (binary, bytes). |

Totals, the five roots:

| Verdict | Text lines | Binary bytes |
|---|---:|---:|
| keeps | 2,916 | 607 |
| keeps, shrinks | 11,828 | 0 |
| moves | 1,854 | 5,934 |
| retires | 22,072 | 176,570 |
| unknown | 1,626 | 0 |
| all | 40,296 | 183,111 |

Totals, outside the five roots:

| Verdict | Text lines | Binary bytes |
|---|---:|---:|
| keeps | 4,726 | 80,200 |
| keeps, shrinks | 226 | 0 |
| moves | 870 | 0 |
| retires | 5,992 | 0 |
| unknown | 0 | 100,502,402 |
| all | 11,814 | 100,582,602 |
