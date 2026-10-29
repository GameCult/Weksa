# Map: interlingua lowering (campaign `weksa-lowering`)

Prose map for the campaign whose target is
[interlingua-lowering.md](interlingua-lowering.md). Rulings, questions,
follow-ups and cut specs live in the Eureka mind under `weksa-lowering:`; this
page is the model page and the cut sequence, and cites them by label. Written
by Imagination for the Eureka-area Self, 2026-10-29.

## The design in one paragraph

The interlingua spec has two layers. The **schema** is Rust types: packet
families, node ids, role slots, value kinds. It changes rarely and by commit.
The **inventory** is data in CultCache: the concepts, frames, roles, relation
types, speech acts and nuance features (hedge, certainty, warmth, irony and
whatever the loop finds missing) that the schema's slots name by id. Every open slot in a packet is an inventory id; a packet's only free text is
form-bounded referent names and the trace. No door judges text for leakage:
what each model call may see, and what a score may count, own that
(`weksa-lowering:ruling:leak-owned-by-flow-and-scorer-3`). The golf loop improves the spec
mostly by growing and refining the inventory, which is fast; schema changes are
the slow path. Concepts are open-class and grow forever; the other families are
closed-class, and convergence is measured on them
(`weksa-lowering:ruling:convergence-test`). This split was
`spec-is-schema-plus-inventory`, now carried by `leak-owned-by-flow-and-scorer-3`.

## Model page

Every persistent kind, what names it, its life, and who decides. "Derived"
means no store holds it and nothing writes it.

| Kind | What names it | Lifecycle | Who decides |
|---|---|---|---|
| Packet | Key `packet:<16 hex>`, an opaque digest of item, inventory revision and encoder (the item key lives in `provenance.corpus_item`, which the lowerer never sees); CultCache type `weksa.interlingua_packet`, carrying `schema_version` and `inventory_rev`. Every id inside is an opaque form type (`n<k>` nodes, `encoder:<16 hex>`, canonical `<family>:<n>`). From cut `speaker-state` (schema v2) it also carries the utterance **stance**: the speaker's felt affect and restraint, as inventory features of slot `Stance` (see Speaker state below) | Minted by an encoder for one corpus item under one spec version; immutable; a new spec version means a new packet, never an edit. Admitted only through `PacketStore::admit`, which validates against the inventory store at the packet's revision | The encoder writes; the form types refuse malformed ids at construction and load, the validator refuses broken references and over-capacity packets: a feature at most once per slot, at most 64 nodes (`typed-packet`); nothing else writes packets |
| Schema version | `weksa.interlingua_packet.v<N>` constant in `crates/weksa`; published JSON Schema at `schemas/cultnet/` | Bumped by a commit that changes the Rust types; the published schema is regenerated in the same commit and a test fails on drift | A cut (Hands, under a spec); never the loop |
| Inventory entry | Key `<family>:<n>`, `n` canonical decimal >= 1 (families: concept, frame, role, relation, speech_act, feature, referent_kind, extension); type `weksa.inventory_entry`, with a bounded gloss, value kind (option labels bounded), the slots a feature may sit in (`Discourse`, `Referent`, `Predication`, `Relation`; from cut `speaker-state` also `Stance`, `Speaker`, `Scene`), `introduced_rev` and optional `retired_rev`. Speaker axes, scene relations and affects are features of those slots, so the loop grows them like any closed-class family and convergence covers them | Added or retired by an inventory revision; never edited in place (a changed meaning is a retire plus an add); a retired id still resolves for packets of earlier revisions | The loop's accepted rounds, whose minting call the context builder assembles from Train items only; Self on merge to main. Admission checks form, not text |
| Inventory revision | Key `rev:<n>`, type `weksa.inventory_revision`, with parent, change list and `frozen` | Appended per accepted round; exactly one revision is ever marked frozen, by the convergence test | The loop's accept rule appends; the convergence test freezes; Self records the freeze as a ruling |
| Target profile | Locale or language-project id (`en-US`, `ja-JP`, a conlang's slug) plus a revision `profile:<language>:<n>` | Authored, revised by commit or by an accepted round that changed the lowering; each lowering names the revision it used | Profile authors and accepted rounds; never the lowerer's defaults (`persona-drives-register`) |
| Language ontology entry (cut `language-ontology`) | Key `onto:<language>:<n>`; type `weksa.ontology_entry`: conditions over feature values (stance, speaker, scene, packet features) and community ids, and the surface choice they select (layer: lexicon, grammar, discourse, phonology, orthography; a bounded descriptor and at most three bounded examples); introduced and retired by a profile revision | Added or retired by a profile revision, never edited; converted from `data/target-language-ontology/*.yaml`, which is deleted in the same cut so the YAML is not a second owner | The language's owner (the operator for her settings' languages) and accepted rounds; an ontology-minting call is assembled from Train items only, like an inventory-minting call, and its text is in the scorer's leak diagnostic. Selection is deterministic code (`select(profile, stance, speaker, scene)`); the lowerer realizes the selected choices and the trace lists their ids (`lowering-traced`) |
| Community (cut `language-ontology`) | Key `community:<slug>`; type `weksa.community`, converted from `data/cultural-ontology/cultures/` and `referents/` (YAML deleted in the same cut) | Revised by commit; a speaker names the communities it belongs to by id | The cultural ontology's author (owner-attested evidence only; Discord-only content stays deleted, `discord-only-content-deleted`) |
| Speaker (cut `speaker-state`) | Key `speaker:<slug>@<n>`; type `weksa.speaker`: stable axes as feature values of slot `Speaker` (origin, socioeconomic standing, age cohort, education and whatever the loop adds), at an inventory revision, and an `Annotator` | A new revision appends, never edits; a corpus item and every prior turn name a speaker slug, and a round records the speaker revisions it read | The annotator it names: `PersonaSnapshot` (a projection of a `gamecult.persona_state.v0` document, naming its source document and `updatedAt`), `Operator`, `WorkMetadata` (a public-domain work's own cast list and narration), or `Model { family, generator }` (the 2b `ModelFamily`) |
| Scene (cut `speaker-state`) | Key `scene:<group>@<n>`, the item's split group; type `weksa.scene`: participants (speaker slugs) and, per ordered pair, relation features of slot `Scene` (relative status, intimacy, in-group, audience and whatever the loop adds), at an inventory revision, and an `Annotator` | As Speaker. Keyed by the group, so a scene never straddles the split | As Speaker |
| Corpus item | Key `item:<source-slug>:<n>`, `n` canonical decimal >= 1 (no sign, no leading zero, from cut 2b); type `weksa.corpus_item` with text, language, speaker, split group, prior turns, source. Source `ModelWritten` carries a `ModelFamily` enum, the generator string as the fixture wrote it, and a prompt digest (cut 2b). The item carries no speaker state of its own: its speaker slug and group name the Speaker and Scene documents | Admitted once; the whole item is immutable (re-adding an identical item is a no-op, any difference is refused; a correction is a new item that `supersedes` the old); text already held under another key is refused unless the new item supersedes that key (cut 2b); withdrawn, never deleted, so old scores stay explainable | Importers and `weksa corpus add`, under `corpus-cast-plus-public-domain`; the source enum has no variant for Discord or any training-forbidden source (`corpus-clean`) |
| Split assignment | Derived: `split(scheme, item.group)` | Fixed for a scheme; a new scheme is a new constant, never a reassignment. A model-written item whose group derives test is in no split; a superseding item keeps its predecessor's group | The hash function. Nobody assigns by hand, so there is no writer to forbid |
| Loop round | Key `run:<label>:round:<n>`; type `weksa.loop_round` with spec version, profile revisions, model ids and prompt digests, sampled train items, proposed change, validation scores before and after, outcome | Opened, scored, closed accepted or rejected; immutable once closed. Lives in the run store on Yggdrasil | The loop driver's accept rule (validation composite improves past the measured noise floor and no metric regresses past tolerance) |
| Score | Key `<lowering-key>:<metric>@<metric-version>`; type `weksa.score` | Immutable; a new metric version scores again under a new key | The scorer, deterministic given the metric version and the embedder's model id. Cut `scorer` computes scores as values; cut `golf-loop-en` stores them, because the lowering keys they hang from arrive with the loop |
| Lowering trace | Key `lowering:<packet-key>:<profile>@<rev>:<lowerer-digest>:<sample>`; type `weksa.lowering` holding the surface text and its trace together | Immutable. A surface text cannot exist without its trace, because they are one record (`lowering-traced`) | The lowerer emits; the lowerer sees the packet without its `trace` (the input type has no such field), never the corpus item |
| Conlang lexicon entry | Key `lex:<language>:<concept-id>`; root form, morphology class | Root placed at its concept's embedding when the concept first needs a form in that language; stable once accepted; diachrony derives later forms, it does not overwrite | The language project's owner: the operator for her settings' languages, the loop proposing roots for her review |
| Conlang grammar and phonology data | `<language>/grammar`, `<language>/phonology` documents per language project | Revised by commit; every lowering names the revision | The language project's owner |
| Training pair (later) | Key `pair:<frozen-rev>:<item-id>:<profile>` | Created only by re-encoding the whole corpus under the frozen inventory revision; never before the freeze (`staged-distill-every-language-equal`); split inherited from the item | The re-encode job, which can only be constructed from a frozen revision |

## Authority map (the packet boundary)

- Owner: the validator in `crates/weksa` owns whether a packet exists.
- Inputs: the packet, the inventory at the packet's revision.
- Outputs: an admitted packet, or a typed refusal naming the rule.
- Derived state: split assignment (from the item's group); the lowering view
  of a packet (the packet minus its trace).
- Forbidden writers: the Node daemon's `decomposeThoughtIntoInterlingua` and
  `normalizeInterlinguaPacket` (parked, cut `park-node-body`); the Metame
  harness's packet-like YAML (parked); any loop step that writes a packet
  without the validator.
- Shared paths: the loop's encoder, the CLI, tests and the future live
  lowering all admit packets through the same `validate` call.
- Deletion line: the Node body goes before the crate lands, so no second packet
  shape survives on main.

## Speaker state (`persona-drives-ontology`)

Operator, 2026-10-29: "The specific choice of words of a character can tell you
exactly where they were from, their socioeconomic status, their relative status
within the scene, and both how they feel and how restrained they are being with
those feelings. The linguistic ontology holds this data and drives it from the
persona." Speaker state is three layers, and each lives where its writer and
lifetime put it:

| Layer | Examples | Writer | Lifetime | Lives in |
|---|---|---|---|---|
| Stable axes | origin and community, socioeconomic standing, age cohort, education | the persona (live) or an annotator (corpus) | the speaker's | `weksa.speaker` |
| Scene relations | relative status, intimacy, in-group or out-group, who overhears | the scene (live) or an annotator (corpus) | the scene's | `weksa.scene` |
| Utterance stance | felt affect, restraint | whoever writes the utterance: the persona agent (live), the encoder (corpus) | the utterance's | the packet (`stance`) |

**Affect and restraint live in the packet, as stance, not in a speaker-state
document.** A per-utterance speaker-state record would have the packet's key,
the packet's writer and the packet's lifecycle (minted once per utterance under
one spec version, immutable), so it would be a second document under one
authority; it collapses into the packet. It also has to be golfed: in the loop
the encoder reads felt affect and restraint off the corpus line, as it reads
the meaning, and the loop improves that reading round by round. A fixed
annotation could not be golfed. Live, the agent writes its own packet
(`campaign-direction`), so the persona drives stance through it. The stance is
a separate section from the meaning (`referents`, `predications`, `relations`):
a feeling the speaker holds back is not a claim, and `packet-owns-meaning`
still forbids a lowering to add one. "I am angry" said outright is a
predication; anger leaking through clipped syntax is stance.

The stance carries **felt** affect and restraint, never displayed affect. How
much of a held-back feeling shows, and how, is a cultural display rule, and
display rules differ by culture (Ekman and Friesen 1969; Matsumoto's
cross-cultural display-rule studies; from recall, not a search this pass). If
the packet carried displayed intensity, the English reading would be baked in
and a ja-JP lowering would apply restraint twice. So each language's ontology
maps felt affect, restraint, scene and speaker to its own surface, which is
`staged-distill-every-language-equal` applied to affect. Whether restraint is
one grade per utterance or one per felt affect is the `speaker-state` spec's
call; the loop can split it later if one grade loses.

**Demotions.** `formality`, `warmth` and `intensity` (seed features 8, 9, 10,
slots Discourse and Predication) are no longer packet owners. Formality is
derived from scene relations and stable axes by the target language's
ontology; displayed intensity is derived from felt affect and restraint; warmth
toward the addressee is a felt affect in the stance. Cut `speaker-state`
removes them from the seed (no committed packet store exists and nothing
outside the seed and the standard names them; Body facts), so no packet can
set formality beside the ontology. `hedge`, `certainty` and `irony` stay: they
are the speaker's epistemic and pragmatic stance toward the content,
language-neutral, not register.

**Corpus items carry speaker state as typed ontology values, never free text.**
The item names its speaker slug and group; the `Speaker` and `Scene` documents
hold feature values of the `Speaker` and `Scene` slots (grades, option labels,
flags, and later community ids), validated against the inventory by the same
feature check the packet uses. No field is free text, so no wording can reach
the lowerer through annotation (`no-surface-leak`). Cast items: the speaker
document is a `PersonaSnapshot` of the cast member's persona state where the
standard has the value, and annotated where it does not (below). Annotations of
stable axes read the whole work or persona, so a speaker's axes are not drawn
from one scored line; a scene's relations read the scene, which includes its
scored lines, and are bounded typed values, as a live scene would supply.

**Persona state today has no field for origin, community or socioeconomic
standing.** `gamecult.persona_state.v0` (Body facts) carries open trait maps
(`activationProfile.voiceStyle` and five more), `moodDimensions`, `socialBonds`
(trust, tension, intensity) and `statusReads` (authority, trust and others).
So a `PersonaSnapshot` projects mood-like and relation-like values, and the
stable axes the standard lacks are annotated. Whether the standard gains a
typed speech-community reference or Weksa keeps the persona-to-community
binding (`data/cultural-ontology/persona-stacks/`) is decided when
`live-lowering` is mapped (follow-up `persona-state-speech-community`); the
corpus path needs neither. Putting Weksa feature ids into the open
`voiceStyle` trait map is rejected: it would make an extension field portable
authority, which the standard forbids.

**Public-domain annotation and the annotator ceiling.** A public-domain play
has tens of speakers and scenes, so annotation is per speaker and per scene,
not per line, and costs nothing per round. Stable axes come first from the
work itself (`WorkMetadata`: the cast list, narration, stage directions). The
ceiling: the loop can learn surface choices only as well as the speaker and
scene values describe the line, so annotation errors become ontology errors (a
servant labelled gentry teaches the ontology the wrong deference). And when a
Claude annotator, a Claude encoder and a Claude lowerer share one family's
idea of what low status sounds like, the loop agrees with itself and scores
the agreement: the round-trip inflation the target already records. Every
annotation names its annotator, so the scorer can report a test composite over
operator-attested annotations alone. Who annotates is question
`pd-speaker-annotation`.

## Scoring (`golf-metric-automatic`, `conlang-scoring`, `meaning-metric-embedding-cosine`)

- English against its corpus item: chrF (character n-gram F-score, char order
  6, beta 2, spaces removed; parity vectors against sacrebleu 2.4.3 in Body
  facts) and meaning cosine (sentence embeddings from qwen3-embedding:0.6b,
  served by Ollama on Yggdrasil; the adapter sends `options.num_thread`, Body
  facts). Probe on one reference: hedged paraphrase
  0.948, flattened claim 0.643, register-shifted 0.720, negation 0.496.
  BERTScore proper needs per-token transformer embeddings and a torch stack;
  it is deferred, not adopted.
- Every language by the same meaning cosine: the embedder reads the output
  directly when it can (natural languages, since the embedder is multilingual);
  for a conlang it reads the deterministic gloss made from the language's own
  lexicon (Weksa owns the lexicon, so reading it back needs no model). Plus
  lexicon adherence for a conlang: the share of tokens that parse against its
  lexicon and morphology.
- A style axis follows as its own cut, behind the same scorer port.
- Names and leak (`leak-owned-by-flow-and-scorer-3`): no packet field carries
  free text, so the scorer masks nothing. A referent names an entity by id
  (`EntityId`, `entity:<slug>`); the entity store (`weksa.entity`) holds each
  entity's forms per language, citation form first, then inflected or
  particle-bound forms (data, as the inventory is). Entities are minted at
  intake: cast entities from the cast fixtures, public-domain entities by the
  importer, and in live use by the persona before it names one. A corpus item
  carries its name spans (byte ranges of its text, each naming an entity and
  equal to one of that entity's forms in the item's language), and its speaker
  and prior-turn speakers are entity ids. The packet store refuses a packet
  whose referent cites an entity the store lacks, and a packet with a
  `corpus_item` that cites an entity neither annotated on that item nor one of
  its speakers: an id-set check, not a text rule. The scorer reads only the
  output and the reference and scores the whole of each. An honest name earns
  its credit ("Brokkr came back." against itself is 100), a wrong one loses it,
  filler costs what filler costs (Soul scorer s2: "Darling, I am truly not
  angry, darling." scores 46.84 whole; the output-only mask, fed the filler as
  a name, had raised it to 86.78), and no rule depends on script or word
  segmentation, so Thai, Korean particles and Turkish or Finnish suffixes are
  scored by the same character n-grams as English. Why this is not a leak:
  leakage is information the learner has that it would not have at prediction
  time (Kaufman et al., TKDD 2012). A live persona knows which entity it means;
  the annotation gives the corpus encoder exactly that, fixed at intake before
  any round and written by no loop writer, and the encoder chooses only which
  of the item's own ids to cite. The free text left is inventory labels and
  glosses (Train-only, the context builder's) and the trace (never lowered or
  scored). Diagnostics, reported and never gates: max pairwise chrF of minted
  text against held-out items; novel coverage pooled per order, for n = 1, 2
  and 3 separately, the covered share of all novel n-grams (word n-grams of a
  held-out item that occur in no Train item) over a split's items, with the
  worst item at n >= 2. Keys split on `_`, `.` and lower-to-upper case
  transitions as well as UAX #29 boundaries, so a snake_case `OptionLabel`
  reads as words. Honest glosses of single words saturate n = 1 (the 54 seed
  glosses read max 1.000, s2), so the leak signature is pooled n >= 2.
  Costs: a live persona mints before it names (one typed write, which also
  buys coreference and one rendering per language); public-domain intake
  annotates name spans (follow-up `pd-name-spans`, annotator per
  `pd-speaker-annotation`); the lowerer inflects a name from its stored forms
  or adds one, traced. Prior art (recall): AMR keeps `:name` strings inside
  the graph beside a `:wiki` link, and AMR-to-text systems copy those strings;
  Abstract Wikipedia renders constructors over Wikidata item ids, with labels
  per language and Lexeme forms for inflection; Grammatical Framework lexicons
  hold proper names (`PN`) with per-language inflection tables; referring
  expression generation (Reiter and Dale 2000; the GREC tasks, 2008-2010)
  chooses a name, pronoun or description from an entity id and the discourse.
  Rejected paths: text rules at the inventory and packet doors (Soul
  packet-store s2); any mask of names in the scored texts: masking reference
  and output (s1: "Kick Bucket" took chrF 53.73 to 100) and masking the output
  only (s2: named filler took chrF 46.84 to 86.78, cosine 0.728 to 0.886);
  equal-length placeholder masking (bounds chrF, not cosine, and still pays
  nothing for an honest name); character 6-gram coverage (one-word glosses
  read 0.000); max-over-items coverage (saturates on honest runs).

## Body facts (probes of 2026-10-29, map pass for speaker state, 2b and scorer)

- Weksa main `28284cc`: `cargo test -p weksa` on Yggdrasil (ygg-verify, rust
  image) 73 passed. `weksa corpus dump` holds three items: `item:brokkr:1`
  (group `brokkr-edda-voice`, Validation, `ModelWritten { model_family:
  "local-source-aware-draft" }`), `item:nibu:1` (group `nibu-proof-001`, Test,
  `CastAuthored`, fixture
  `examples/english-lowering/nibu-proof-001/lowered-output.yaml`, which records
  no author) and `item:nibu:2` (same group, excluded, `ModelWritten {
  model_family: "unrecorded-agent-run" }`; that string exists only in
  corpus.cc). The Brokkr fixture's generator was `codex-local-source-aware-draft`
  at `4197123` and reads `local-source-aware-draft` on main.
- `CorpusStore::add` compares only `text` (`store.rs:166-171`): re-adding a key
  with the same text and a different source returns `Unchanged` and writes
  nothing, so a relabel through `add` silently does nothing. `key_slug`
  parses `n` with `u32::parse`, which accepts `01` and `+1`.
- Inventory rev 1 (seed): features 6-11 are hedge, certainty, warmth,
  formality, intensity (Grade) and irony (Flag), slots Discourse and
  Predication. Nothing outside the seed and `docs/interlingua-standard.md:179-180`
  names warmth, formality or intensity; no committed packet store exists.
- Yggdrasil's Ollama (`127.0.0.1:11434`) is the Docker container `ollama-cpu`,
  Huginn's embedder (runbook `gamecult-ops/runbooks/huginn-yggdrasil.md`,
  ruling `eureka-body:ruling:huginn-embedder-yggdrasil-primary`). `/api/embed`
  with an `input` array returns unit-norm 1024-dimension vectors. Under host
  load (load average 18-24 from ygg-verify rustc jobs) and no `num_thread`: a
  batch of 6 took 21.5 s and 29.8 s, a batch of 32 timed out at 120 s. With
  `options.num_thread` 8: a batch of 6 in 2.1 s, a batch of 32 in 5.8-6.4 s
  (0.18-0.20 s per item); `num_thread` 4: 8.8 s for 32. Cosine of "Take your
  hand off that control." against its lowercased, unpunctuated form: 0.943.
  One server serves Huginn and the loop, so a 32-item batch holds it about 6 s.
- chrF parity: sacrebleu 2.4.3 `sentence_chrf(hyp, [ref])` and a Python
  reimplementation (orders 1-6, per-order precision and recall averaged over
  the orders where both sides have n-grams, F-beta with beta 2, spaces
  removed) agree to 4 decimals on six pairs (reference / hypothesis / score):
  "Take your hand off that control unless you want your nervous system to become my least useful circuit diagram." /
  "Take your hand off that control, unless you'd like your nervous system to be my least useful wiring diagram." / 74.4498;
  "The command has no receipt. It is not truth." / "The order carries no receipt, so it is not true." / 45.7164;
  "I am not angry." / the same / 100.0;
  "Forgive me, sir." / "Get out." / 3.9683;
  "その制御に触るな。" / "その操作盤に触らないで。" / 14.2931;
  "Well, aren't you a sight for sore eyes, love." / "" / 0.0.
- Scorer s1 follow-up probe (sacrebleu 2.4.3 in python:3.12-slim on Yggdrasil;
  qwen3-embedding:0.6b, one 8-text call). sacrebleu strips every whitespace
  char before gramming: hypothesis "a	b c", "a b", "ab",
  "ab", "a b c", "line one
line two", "その　制御" against the
  same text with that char removed (or "line one line two") all score 100.0;
  "a​b" against "ab" scores 45.4545 (zero-width space is not
  whitespace). Name scoring, output masked and reference whole (hypothesis /
  reference / chrF / cosine): "He is going to die tomorrow." / "He is going
  to kick the bucket tomorrow." / 53.7347 / 0.9081; "He is going to the
  tomorrow." (named or omitted) / same / 55.9583 / 0.8246; "Not ever again."
  / "Never again." / 85.1384 / 0.9169; "." (named) / same / 11.1111 /
  0.6456; "came back." / "Brokkr came back." / 56.6137 / 0.7364; "Kestrel
  came back." / same / 51.3670; "not go." / "Will will not go." / 33.6248.
- `gamecult.persona_state.v0` (`F:\Projects\Epiphany\schemas\cultnet\gamecult.persona_state.v0.schema.json`):
  required `activationProfile` (six open trait maps of mean, plasticity and
  current activation) and `affect` (`needs`, `socialBonds`, `statusReads`,
  `moodDimensions`, `socialBiases`, `doctrineStances`);
  `presentation.voiceSummary` is free text; no field names origin, community
  or socioeconomic standing; `extensions` are non-authoritative by the
  schema's own description.

## Where things live and run (`rust-single-crate`, `loop-runs-on-yggdrasil`)

- Code: one Cargo workspace at the repo root, one package `crates/weksa` (a
  library and the `weksa` binary). CultCache through `cultcache-rs` pinned by
  git rev, as Huginn pins it. No daemon: the loop is a batch command. A daemon
  is earned later only by the live lowering service, if isolation protects a
  named invariant there.
- Stores: `data/interlingua/inventory.cc` and `data/corpus/corpus.cc` in the
  repo (small, reviewed with `weksa dump`); run stores (rounds, lowerings,
  scores) on Yggdrasil under the loop's state root, not in git.
- The loop runs on Yggdrasil as a detached unit with a log and a status record.
  Starfire runs none of it. Model calls go through a `ModelPort`; its first
  adapter drives `claude -p` on the operator's subscription. Never OpenAI
  (`eureka-body:ruling:no-return-to-openai-supplier`), never Discord data
  (`gamecult-chat:ruling:discord-taints-training`).

## Convergence (`convergence-test`)

The spec has converged when, on a panel of en-US, ja-JP and the first conlang,
for 20 consecutive rounds no accepted change to a closed-class inventory family
or the schema raised the validation composite past the noise floor, and at that
checkpoint the test-split composite is within the noise floor of validation.
The test split is read only at checkpoints. Self then records the freeze, and
the training set is the whole corpus re-encoded under the frozen revision.

## Cut sequence

One line per cut; specs exist for the cuts about to run (2b and 3, siblings).

1. `park-node-body`: park the Node daemon, MiMo command, RUDP shims, launchers and the Discord-sourced Metame harness at tag `parked/weksa-node-body`; README front door tells the truth. Landed.
2. `packet-store`: Rust crate with packet, inventory and corpus types, the validator, the derived split, the stores and the `weksa` CLI. Landed at `28284cc`.
2b. `packet-store-corpus`: corpus labels tell the truth (`corpus-labels-tell-truth`): `ModelFamily` and the generator string on `ModelWritten`; `item:nibu:1` ModelWritten (Unrecorded); `item:brokkr:1` and its fixture carry `codex-local-source-aware-draft`, family OpenAi; intake refuses aliased keys, a whole item changed under its key, and a held item's text under a new key; corpus.cc reseeded. Speaker state is not in 2b: it lives in Speaker and Scene documents and the packet's stance, not on the item. Waits on 2. (spec)
2c. `entity-store`: `ProperName` and `Referent.name` deleted; `EntityId`, `Referent.entity`, the `weksa.entity` document and its store; corpus name spans and entity-id speakers; the packet store's entity checks; seed entities and spans for the cast items; schema v2. Waits on 2b. (spec r1)
3. `scorer`: chrF, meaning cosine behind an `Embedder` port (Ollama adapter with `num_thread`), leak diagnostics, aggregation by derived split; library only, `weksa score` comes with the loop. h2 landed the r2 fix pass on its branch; r3 deletes the name mask (the scorer reads output and reference only) and pools novel coverage per order over word keys that split joined labels. Waits on 2c for merge order. (spec r3)
4. `speaker-state`: inventory slots `Stance`, `Speaker`, `Scene`; packet schema v3 with `stance` (felt affect, restraint); formality, warmth and intensity removed from the seed and the standard; `weksa.speaker` and `weksa.scene` documents with `Annotator` (reusing 2b's `ModelFamily`), validated by the packet's feature check; Speaker documents keyed by the entity ids of 2c; cast speakers and scenes for the three items. Waits on 2b and question `pd-speaker-annotation` only for the annotator variants it admits.
5. `language-ontology`: `weksa.community` from `data/cultural-ontology/` (owns follow-up `edda-referent-yaml-parse`), `weksa.ontology_entry` for en-US and ja-JP from `data/target-language-ontology/` (YAML deleted), speaker communities, deterministic `select`, the lowering view (packet without trace, speaker, scene, selected entries). Waits on 4.
6. `pd-importer`: public-domain play and fiction dialogue into the corpus with speaker, scene group, edition provenance, and Speaker and Scene annotations by the ruled annotator. Waits on 4, follow-up `gutenberg-terms` and question `pd-speaker-annotation`.
7. `golf-loop-en`: the loop driver, the Train-only context builder for inventory- and ontology-minting calls (follow-up `loop-context-builder-train-only`), the packet store bound to one inventory store (`packet-store-inventory-binding`), `ModelPort` with the `claude -p` adapter, encoder (packet with stance) and lowerer prompts, round records, Score documents, the composite and accept rule, `weksa score`, the detached unit on Yggdrasil. Waits on 3, 5, 6 and follow-up `claude-on-yggdrasil`.
8. `style-axis`: a style-embedding score behind the scorer port. Waits on 3.
9. `ja-panel`: ja-JP into the loop's panel. Waits on 7.
10. `conlang-lexicon`: the first conlang's lexicon placed at concept embeddings, its morphology data, its ontology entries and the gloss reader. Waits on 3, 5 and question `first-conlang`.
11. `convergence-freeze`: the convergence test, the freeze, the re-encode into training pairs. Waits on 7, 9 and 10.
12. `distil`: fine-tune a lowerer on the frozen training set. Waits on 11 and a host question asked then.
13. `live-lowering`: agents emit packets with stance; Weksa reads speaker state from persona state (follow-up `persona-state-speech-community`) and lowers for the reader. Waits on 11.
