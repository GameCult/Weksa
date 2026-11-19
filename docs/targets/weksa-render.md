# Target: Weksa renders packets into languages by dictionary and grammar

Draft by Imagination, 2026-11-20, for the Weksa Self (session
`weksa-self-20261119`). The operator sees this and its map
([weksa-render-map.md](weksa-render-map.md)) before anything is deleted. Campaign:
`weksa-lowering` until question `render-campaign-home` is ruled (recommended: a
new campaign `weksa-render`).

This replaces [interlingua-lowering.md](interlingua-lowering.md), whose name
and method (a model lowers, a golf loop scores) are retired. The path is new
because the old name describes the retired method (idea
`render-map.keep-old-target-path`, Recorded).

## Rulings this stands on

- `deterministic-conlang-pivot` (operator, 2026-11-19): Weksa is a conlang
  workshop whose languages render from packets by dictionary and rules,
  deterministically, consistently enough that a person could learn them. No
  model renders.
- `natural-language-calibration` (operator, 2026-11-19): the same renderer is
  first built and checked on English and Japanese against real text such as
  the Dao De Jing translations. That is the operator's evidence that the
  conlangs are decent language.
- `gf-adopted` (operator, 2026-11-20): Grammatical Framework renders and parses
  every Weksa language. Weksa's Rust owns packet-to-tree planning, the
  dictionary, phonology, coinage and sound change. Register and other
  relation-driven choices are made at abstract level.
- `first-conlang-rust-to-dust` (operator, 2026-10-29): the first conlang is
  Rust to Dust's imperial language. Its reasons (a unified empire with a formal
  history, built to be relatable) do not depend on the loop, so it stands.
- `campaign-direction` (operator, 2026-10-29) is **superseded in its end and
  method, and its purpose stands.** Its end was agents' interlingua lowered to
  a reader's display language. That end is replaced by rendering conlangs, with
  display languages as the calibration bed. Its purpose was a working body for
  Weksa, and that still holds. Live persona speech in a display language is
  not a consumer here unless the operator keeps it (question
  `render-campaign-home`, option `new-campaign-live-consumer`).
- Standing and refined: `stance-in-packet-speaker-scene-documents` (felt affect
  and restraint stay in the packet; stable axes and scene relations stay in
  speaker and scene documents). `register-stays-per-speaker` (no register value
  in the packet; the planner now derives it into the tree, where it was the
  language ontology's to derive). `rust-single-crate`.
  weksa-body's `rtd-imperial-*` rulings move from "the imperial ontology" to
  the imperial language definition and its concrete grammar.

## Ends

1. **Constructed languages for the operator's worlds** (Rust to Dust first,
   then Zyphos) render from interlingua packets, deterministically, with
   grammar and vocabulary consistent enough to learn.
2. **English and Japanese render through the same machinery** as the
   calibration bed, checked against published translations of the Dao De Jing.
3. **Words and names are coined deterministically** from each language's
   definition. Zyphos names have been owed since May (weksa-body).

## The pipeline

```text
packet (validated)  +  speaker and scene state  +  entity store
   │
   ▼  planner (Rust, language-neutral, deterministic)
abstract tree of the shared GF abstract syntax `Weksa`
   │   (register, relation and stance categories are nodes of the tree)
   ▼  checked (pgf_check_expr), then linearized by the language's concrete
token string  ──► parse back in the same language: original tree among the parses, or refuse
   │
   ▼  orthography (the language definition's rule: Japanese joins tokens, conlangs romanize)
surface text + trace          ──► optional: pronounce (phone inventory, ruled IPA)
```

The dictionary feeds the grammar: every concept the dictionary knows is a
lexical function of the abstract, and every entry in a language is a lexical
linearization of that language's concrete. Both are generated from the
dictionary and never hand-edited.

## Invariants

- `one-machinery`: every language, natural or constructed, renders through
  the same planner, the same shared abstract syntax, one concrete grammar of
  its own, and one dictionary kind. No language has a private path.
  English and Japanese are calibration because nothing distinguishes them from
  a conlang except where their definition came from.
- `no-model-renders`: no model call sits between an admitted packet and
  surface text, or between surface text and the round-trip verdict. Models may
  draft packets, language-definition proposals and critiques outside the
  render path. Anything they draft reaches a render only through the same
  doors a person uses: `validate` for packets, dictionary admission for
  words, a commit for a definition.
- `abstract-level-choices`: register and every relation- or stance-driven
  choice are nodes of the abstract tree. These include the speaker's standing
  toward the addressee, intimacy, in-group and audience. The planner chooses
  them from the packet's stance and the scene, never per language. Each
  concrete grammar decides how, or whether, its surface shows them. A language
  that does not mark a choice renders both trees alike, and its parse returns
  both. The probe showed this is the only form that parses back
  (`gf-probe.parse-field0`).
- `round-trip`: every render parses its own token string in the same language,
  and the original tree must be among the parses. A render that fails this is
  refused, not shipped. More than one parse is allowed, since that is real
  ambiguity (Japanese has no articles). No parse at all is a grammar fault.
- `render-deterministic`: the same packet, speaker and scene state, entity
  store, dictionary, grammar build and language-definition revision give
  byte-identical text and trace.
- `render-is-pure`: render writes nothing. A concept with no entry in the
  target language is a refusal naming the concept. It is never coined at render
  time (idea `render-map.render-time-coinage`, Recorded).
- `dictionary-append-only`: an admitted dictionary entry is never edited or
  deleted. A correction is a new entry that supersedes the old one. Each render
  trace names the entry ids it used, so an old render stays explainable.
- `coinage-deterministic`: the form coined for a (language, concept) pair is a
  pure function of three things: the language-definition revision, the concept
  id, and the dictionary as it stood (only for collision avoidance). Coining
  the same pair twice gives the same form. Coining a new concept changes no
  existing form, because each pair seeds its own draw rather than sharing one
  random stream.
- `learnable`: a conlang's surface comes from its definition and dictionary
  alone. There are no per-utterance exceptions and no hand-edited surfaces.
  Irregularity comes only from sound-change stages applied to recorded roots.
- `packet-owns-meaning` (kept): no render adds a claim, referent or commitment
  the packet lacks. Where the abstract needs a distinction the packet is silent
  on (definiteness, number), the planner's written, language-neutral default
  decides, and the trace names the default.
- `typed-packet` (kept): the packet is a typed CultCache document with a
  published schema, refused at the door when malformed. Schema v6 drops
  `inventory_rev` and the required corpus item.
- `render-traced`: every text carries a trace. The trace names the packet id,
  the language-definition revision, the grammar build digest, the dictionary
  entry ids, the abstract tree, the planner defaults applied, and the
  round-trip parse count.

## What is explicitly not a consumer

- A model producing surface text in any language: the model-lowerer, its
  prompts, and the language ontology that steered it.
- The golf loop and its machinery: rounds, cycles, the accept rule, chrF,
  meaning cosine, leak diagnostics, Train/Validation/Test splits, and
  distillation training pairs.
- Live persona speech in a display language, unless kept by question
  `render-campaign-home`.
- Parsing English (or any text) into packets. Round-trip parsing compares
  trees and never builds a packet.
- Speech handoffs: MiMo voice design, AquaSynth utterance embeddings, and the
  panphon training handoffs. `weksa pronounce` stays as a phonology readout,
  not as a speech contract.
- Ghostlight Speak's verbatim span (weksa-body `vq-ghostlight-packets`
  recommends Weksa stay downstream).
- Eve.

## Body today (main 1e06f00)

`crates/weksa` is one Rust crate with 13,773 lines of src and 20,101 lines of
tests and test fixtures. Its live authorities are a typed packet (schema v5, 14 fields, every
open slot an inventory id at an inventory revision) and a validator. It also
has stores for corpus, inventory, entity, speaker state and language ontology.
The golf loop (`run.rs`, 4,112 lines) has the model answer encode, lower and
mint prompts through files (`context.rs`, `model.rs`, `encode.rs`), and a
scorer (`score/`) reads chrF and an Ollama meaning cosine. No deterministic
renderer exists: commit #93 retired the English template table and `lower()`,
and `weksa lower` became `weksa pronounce`. The June-era docs
`runtime-authoring-split.md` and `project-model.md` already describe a
deterministic, LLM-free render path; the campaign went around them. The GF
probe (`e413e89`, branch `weksa-lowering/gf-probe`, not merged) ran English and
Japanese on two Dao lines on Yggdrasil. It showed a 34 KB PGF, a 1.3 to 2.1 ms
load, and a round trip with the original tree among the parses on every
parsable cell.

## Prior art

- **GF** (Ranta 2011; RGL, Ranta 2009 LiLT): one abstract syntax, many
  concretes, parse and linearize from one grammar. The RGL has English with
  Dict and WordNet coverage. Japanese has a two-valued Style and no lexical
  keigo (`gf-fit.jpn-politeness`). gf-wordnet has English, Japanese and
  Esperanto concretes (`gf-fit.lexicon`).
- **LinGO Grammar Matrix** (Bender, Flickinger and Oepen 2002; Bender et al.
  2010) generates grammars from typological choices. This is the precedent for
  generating a conlang concrete from its definition (question
  `conlang-grammar-authoring`).
- **Abstract Wikipedia / Wikifunctions** renders language-neutral constructors
  per language. It reports years of per-language grammar cost and the risk of
  English-shaped constructors (recall; `2026-10-29-interlingua-lowering-prior-art.md`).
- **Collaborative Interlingual Index** (Bond et al. 2016): language-neutral
  concept ids (question `concept-identity`).
- **Sound-change appliers** (Rosenfelder's SCA², Lexurgy): ordered rewrite
  stages over phonemes, the form diachrony takes here (recall, not verified
  this pass).

## Where Weksa's Rust ends and GF begins

| Weksa (Rust, `crates/weksa`) | GF (grammars, compiled PGF, C runtime) |
|---|---|
| packet validation; planner from packet, speaker and scene state to abstract tree | abstract syntax: categories and constructors |
| dictionary store; generation of the abstract and concrete lexicon modules | concrete grammars: word order, agreement, inflection, register realization |
| language definitions: phonology, orthography, coinage, sound change, grammar parameters | linearize; parse |
| the one checked FFI wrapper (`pgf_check_expr` before linearize; owned strings, because `gu_string_in` does not copy) | |
| orthography, round-trip gate, trace, `pronounce` | |

Build: the GF compiler is GPL and Haskell, and it is a build tool only
(`gf-fit.licence-compiler`). The PGF is built on Yggdrasil in the Ubuntu 24.04
container the probe proved. It is a derived artifact, not committed, and its
digest is in every trace (idea `render-map.commit-pgf`, Recorded). The runtime
links the LGPL/BSD C runtime. Nothing has been built on Windows yet.

## Open forks (questions in the mind, weksa-lowering)

- `render-campaign-home`: new campaign or continue; live persona speech.
- `gf-grammar-home`: this repo, a separate repo, or split by owner.
- `concept-identity`: ILI, GF WordNet function names, or Weksa-minted ids.
- `conlang-grammar-authoring`: generated from the definition or hand-written.
- `conlang-start`: when the imperial conlang starts relative to calibration.
- `calibration-evidence`: what counts as decent English and Japanese.
