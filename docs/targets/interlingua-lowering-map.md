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
types, speech acts and nuance features (hedge, certainty, irony, stance affects and restraint, and
whatever the loop finds missing) that the schema's slots name by id. Every open slot in a packet is an inventory id; a referent names an entity by id
from the entity store, and a packet's only free text is the trace. No door judges text for leakage:
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
| Packet | Key `packet:<16 hex>`, an opaque digest of item, inventory revision and encoder (the item key lives in `provenance.corpus_item`, which the lowerer never sees); CultCache type `weksa.interlingua_packet`, carrying `schema_version` and `inventory_rev`. Every id inside is an opaque form type (`n<k>` nodes, `encoder:<16 hex>`, canonical `<family>:<n>`). From cut `speaker-state` (schema v4) it also carries the utterance **stance**: the speaker's felt affect and restraint, as inventory features of slot `Stance` (see Speaker state below) | Minted by an encoder for one corpus item under one spec version; immutable; a new spec version means a new packet, never an edit. Admitted only through `PacketStore::admit`, which validates against the inventory store at the packet's revision | The encoder writes; the form types refuse malformed ids at construction and load, the validator refuses broken references and over-capacity packets: a feature at most once per slot, at most 64 nodes (`typed-packet`); nothing else writes packets |
| Schema version | `weksa.interlingua_packet.v<N>` constant in `crates/weksa`; published JSON Schema at `schemas/cultnet/` | Bumped by a commit that changes the Rust types; the published schema is regenerated in the same commit and a test fails on drift | A cut (Hands, under a spec); never the loop |
| Inventory entry | Key `<family>:<n>`, `n` canonical decimal >= 1 (families: concept, frame, role, relation, speech_act, feature, referent_kind, extension); type `weksa.inventory_entry`, with a bounded gloss, value kind (option labels bounded), the slots a feature may sit in (`Discourse`, `Referent`, `Predication`, `Relation`; `Stance` from cut `speaker-state`, `Speaker` and `Scene` from cut `speaker-scene`), `introduced_rev` and optional `retired_rev`. Speaker axes, scene relations and affects are features of those slots, so the loop grows them like any closed-class family and convergence covers them | Added or retired by an inventory revision; never edited in place (a changed meaning is a retire plus an add); a retired id still resolves for packets of earlier revisions | The loop's accepted rounds, whose minting call the context builder assembles from Train items only; `weksa inventory admit` for one model-drafted `EditDraft` outside a run (cap 256, reviewed whole in its cut; same numbering and admission check as the loop); Self on merge to main. `data/original` revs 5 (233 concepts and frames) and 6 (five concepts closing named gaps) were drafted by claude-sonnet-5-5 (Weksa Hands, model-drafted original-bed lexicon from Train and Validation passages); Validation-informed: numbers on this bed measure lowering, never generalization or convergence. Residue after rev 6, not amended (each fresh coverage reader names new gaps; the loop's growth path owns them): simultaneity (ながら), distal deixis (あの), 'first' (まず), counterfactual mood, 'at least', emphatic question; heaven versus sky for the visible 天 stays weak. Admission checks form, not text |
| Inventory revision | Key `rev:<n>`, type `weksa.inventory_revision`, with parent, change list and `frozen` | Appended per accepted round; exactly one revision is ever marked frozen, by the convergence test | The loop's accept rule appends; the convergence test freezes; Self records the freeze as a ruling |
| Target profile | Locale or language-project id (`en-US`, `ja-JP`, a conlang's slug). Until entries grow in rounds there is no profile revision document: a run names the profile it lowered with by the ontology store's path and digest in its `RunRecord` (cut `language-ontology`); `profile:<language>:<n>` revision documents arrive with follow-up `language-ontology-growth` | The ontology store is revised by commit; a run pins it and refuses drift; it likewise pins the speaker-state store (`RunRecord` keys 17 and 18, `Drift(SpeakerState)`, cut `language-ontology-scene`) | Profile authors and accepted rounds; never the lowerer's defaults (`persona-drives-register`) |
| Language ontology entry (cut `language-ontology`) | Key `onto:<language>:<n>`, `n` from 1 per language; type `weksa.ontology_entry` in `data/ontology/ontology.cc`: the language, the revision of the inventory it was admitted through (provenance: beds number revisions independently and a run reads each condition's feature at its own head), a layer (`Lexicon`, `Grammar`, `Discourse`, `Phonology`, `Orthography`, as the door spells them; the mint prompt ends with an `example` line serialized from the draft type itself), at most four conditions (a feature id of slot `Stance`, `Discourse` or `Scene` with `AtLeast`/`AtMost`/`AtMostOrUnset` a grade or `Is` an option or flag; a `Scene` condition reads the relation of the item's speaker to its addressee that `with_scene` selected; slot `Speaker` and community ids wait on follow-up `language-ontology-communities`), a choice of 1 to 200 bytes, at most three examples of 1 to 120 bytes, and an `Annotator`. Whether an entry's text keeps politeness on the scene is a model reader's judgment, owed per change (a mint owes it under `language-ontology-growth`), not a test | Admitted in all-or-nothing batches through `OntologyStore::admit`, immutable, never edited; retire arrives with `language-ontology-growth`. The run pins the store like the entity store (path and digest, `Drift(Ontology)`) and never writes it; slice 1 of growth is outside any run: `weksa loop mint-ontology` drafts entries from the run's Train items of one language into `<root>/ontology-mint/<digest>.answer`, `weksa ontology admit --ontology <copy>` admits them into a file copy, and a new run pins the copy (a run-owned copy waits on follow-up `language-ontology-growth`). Cut `language-ontology-seed` converts by drafting, not by parse: a fresh model drafts each language's seed from that language's YAML and the inventory's feature lines with no corpus text, then deletes `data/target-language-ontology/` and `.weksa/cultural-ontology/` in the same cut | Seed: a labelled model (`Annotator::Model`), no operator gate, never Test evidence (`ja-ontology-owner-answer`); attestation (operator, native reader) appends later. Accepted rounds later, through a mint call from Train items only. Selection is `ontology::select(ontology, language, view)`, called only inside `Prompt::lower`, so no caller picks entries; the prompt prints each selected entry's id, layer, choice and examples. Its text is minted text for the leak diagnostic, which today has no runtime caller (follow-up `leak-diagnostic-in-round`). The store was rebuilt without ja-JP `:14` and `:15` (above-status Scene entries; a paired measure found them net-negative on register, follow-up `ja-scene-entries-lower-register-agreement`), so ja-JP ends at `:13`: an above-status out-group addressee, and an above-status in-group addressee below intimacy 700, select no ja-JP Scene entry (`:12` still selects in-group at intimacy 700 or more regardless of status) until a native reader attests a replacement (`ja-native-reader-scenes`). ja-JP `:5`, `:7` and `:8` were revised by certainty and restraint `AtMostOrUnset` conditions (holds when the feature is unrated or at most the grade; `AtMost` still fails on an unrated feature) in a store rebuild (the store has no retire, so a conditions change is a rebuild), which makes ja-JP's hedging, certainty, affection, restraint and sadness entries disjoint on `:5`/`:6`, `:4`/`:7`, `:6`/`:7`, `:6`/`:8`, and ja-JP `:3` (anger) carries `AtMostOrUnset` hedge and affection bounds, as en-US `:3`, which makes `:3`/`:5` and `:3`/`:7` disjoint; en-US `:3` carries `AtMostOrUnset` hedge and affection bounds, `:5` an `AtMostOrUnset` certainty bound (the clause it held in prose) and `:8` one too (as ja-JP `:7`), which makes `:3`/`:5`, `:3`/`:8`, `:5`/`:6` and `:6`/`:8` disjoint, while `:6`/`:10` stay co-selectable and are kept apart by scoping in `:6`'s text; ja-JP `:9` was reworded in a rebuild to state its question ending as available, so `:6`/`:9`, `:5`/`:9` and `:3`/`:9` stay co-selectable and are reconciled by wording, not by condition |
| Community (cut `language-ontology`, follow-up `language-ontology-communities`) | Key `community:<slug>`; type `weksa.community`, built fresh when the cut is mapped: `data/cultural-ontology/` (draft YAML profiles, read by no code) is deleted from main (cut 5e `delete-cultural-ontology`) and stays in git history before that cut's commit | Revised by commit; a speaker names the communities it belongs to by id | The cultural ontology's author (owner-attested evidence only; Discord-only content stays deleted, `discord-only-content-deleted`) |
| Speaker (cut `speaker-scene`) | Key `speaker:<slug>@<n>`; type `weksa.speaker`: stable axes as feature values of slot `Speaker` (origin, socioeconomic standing, age cohort, education and whatever the loop adds), at an inventory revision, and an `Annotator` | A new revision appends, never edits; a corpus item and every prior turn name a speaker slug, and a round records the speaker revisions it read | The annotator it names: `PersonaSnapshot` (a projection of a `gamecult.persona_state.v0` document, naming its source document and `updatedAt`), `Operator`, `WorkMetadata` (a public-domain work's own cast list and narration), or `Model { family, generator }` (the 2b `ModelFamily`) |
| Scene (cut `speaker-scene`) | Key `scene:<group>@<n>`, the item's split group; type `weksa.scene`: participants (speaker slugs) and, per ordered pair, relation features of slot `Scene` (relative status, intimacy, in-group, audience and whatever the loop adds), at an inventory revision (provenance; a run reads the features at its own head), and an `Annotator` | As Speaker. Keyed by the group, so a scene never straddles the split. `data/original/speaker-state.cc` holds Model-annotated keeper scenes, one per Train or Validation group of both languages (`weksa scene put|dump`); a Test group has none and reads `NoScene` | As Speaker; a model-annotated scene is never Test evidence |
| Corpus item | Key `item:<source-slug>:<n>`, `n` canonical decimal >= 1 (no sign, no leading zero, from cut 2b); type `weksa.corpus_item` with text, language, speaker, split group, prior turns, source. Source `ModelWritten` carries a `ModelFamily` enum, the generator string as the fixture wrote it, and a prompt digest (cut 2b). The item carries no speaker state of its own: its speaker slug and group name the Speaker and Scene documents | Admitted once; the whole item is immutable (re-adding an identical item is a no-op, any difference is refused; a correction is a new item that `supersedes` the old); text already held under another key is refused unless the new item supersedes that key (cut 2b); withdrawn, never deleted, so old scores stay explainable | Importers and `weksa corpus add`, under `corpus-cast-plus-public-domain`; the source enum has no variant for Discord or any training-forbidden source (`corpus-clean`) |
| Split assignment | Derived: `split(scheme, item.group)` | Fixed for a scheme; a new scheme is a new constant, never a reassignment. A model-written item whose group derives test is in no split; a superseding item keeps its predecessor's group | The hash function. Nobody assigns by hand, so there is no writer to forbid |
| Loop round | Key `run:<label>:round:<n>`; type `weksa.loop_round` with spec version, profile revisions, model ids and prompt digests, sampled train items, Validation means and refusal counts (from cut `ja-panel-loop`: one set per language of the run's panel, no pooled mean), per-item Validation scores in corpus order (key 22, numbers only, no item key or text, private to `run.rs` for the paired floor of the cycle (rounds 1 to 4) and of relower; a run from before has none and opens), the digest of the inventory file it was scored under and, for a cycle's candidate round, its cycle `k` | Scored and recorded whole; immutable. A candidate round is put in one batch with its Decision. Lives in the run store on Yggdrasil | `run::round` and `run::cycle` (both through `score_round`) |
| Loop decision | Key `run:<label>:cycle:<k>`; type `weksa.loop_decision` with the base round and revision, the edit prompt's digest and Train keys, the editor's answer digest, the candidate revision, inventory digest and round, the base, candidate and floor readings (chrF, meaning, composite; the floor is the paired floor of round 1 against rounds 2 to 4), the threshold and the verdict (Accepted, or Rejected for edit-parse, edit-admit, no-gain, regressed chrF, meaning or composite); digests and numbers, no text | Immutable. The latest Accepted one names the run's head inventory (`<root>/cycle-<k>/inventory.cc`, its candidate digest, its candidate round); with none the head is the copy `create` made at `<root>/inventory.cc`. Lives in the run store on Yggdrasil | `run::cycle`'s accept rule, `run::decide`: the validation composite gains more than max(noise floor, `LADDER_STEP`), no metric falls past max(its floor, one step), the composite does not fall past max(its floor, one step), and a non-finite reading rejects (the one regression test, `run::regression`); differences compared in billionths so a gain of exactly one step is a tie. From cut `ja-panel-loop` the readings, floor and threshold are kept per panel language and `run::regression` runs per language and is the one regression test (`decide` adds the gain test for a one-language panel): any language regressing rejects (verdicts `regressed-chrf`, `regressed-meaning`, `regressed-composite`), otherwise a one-language panel accepts as `decide` does and a panel of two or more accepts when the sum of composite deltas exceeds one `LADDER_STEP` (billionths). Given up: strict Pareto non-regression; a language may lose up to its own noise floor if the panel nets more than a step, because zero tolerance ratchets every base to a favourable noise draw and strangles the loop. Termination: each accept raises the composite sum by more than a step and composites lie in -0.5 to 1, so a run accepts at most 1.5 * languages / `LADDER_STEP` edits |
| Score | Key `<lowering-key>:<metric>@<metric-version>`; type `weksa.score` | Immutable; a new metric version scores again under a new key | The scorer, deterministic given the metric version and the embedder's model id. Cut `scorer` computes scores as values; cut `golf-loop-en` stores them, because the lowering keys they hang from arrive with the loop. Lands with follow-up `golf-loop-score-docs`, in the round's directory; nothing in the cycle reads them |
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
binding (the deleted `data/cultural-ontology/persona-stacks/brokkr.yaml` was its one draft) is question `persona-speech-community-home` (recommended: a Weksa-owned
binding; follow-up `persona-state-speech-community`), decided when
`live-lowering` is mapped; the
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
  on extended grapheme cluster boundaries of its NFC text and equal to one of that
  entity's NFC forms in the item's language; an entity holds at most 8 languages
  of BCP 47-form tags, 8 forms each, a form at most 64 bytes with no invisible format character), and its speaker
  and prior-turn speakers are entity ids. The packet store refuses a packet
  whose referent cites an entity the store lacks, and a packet with a
  `corpus_item` (required, typed `CorpusItemId`) that cites an entity neither annotated on that item nor one of
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
  worst item at n >= 2. A word is a run of letters, digits, marks and
  joiners; any other character, and a lower-to-upper case transition, ends it,
  so `kick:the:bucket` and a snake_case `OptionLabel` read as words. Han, kana,
  Thai, Lao, Khmer and Burmese write no word boundaries and the scorer holds no
  dictionary, so a run of them is one word (per-character words would give
  every honest gloss shared bigrams); a leak there shows at n = 1 and in chrF. Honest glosses of single words saturate n = 1 (the 54 seed
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

- Weksa main `869c82b` (packet-store, 2b corpus labels, 2c entity store, scorer
  landed; packet schema v3). Annotators land with their consumers: `Operator`
  and `Model { family, generator }` with cut `speaker-scene` (the cast seeds are
  Model-annotated and an operator attests by appending an `Operator` revision),
  `WorkMetadata` with `pd-importer`, `PersonaSnapshot` with `live-lowering`.
- UAX #29 word probe (unicode-segmentation 1, scratch crate, 2026-10-29):
  `unicode_words()` counts (whitespace count in brackets) "Brokkr" 1 [1],
  "Mary Ann Evans" 3 [3], "de la Cruz y Garcia" 5 [5], Thai "สมชาย" 5 [1],
  "สมชาย ใจดี" 8 [2], "山田太郎" 4 [1], "李小龍" 3 [1], "ブロッカー" 1 [1],
  "김민준" 1 [1], "Jean-Luc" 2 [1], "O'Brien" 1, five letters glued by ZWSP 5 [1],
  glued by WJ U+2060 1 [1]. Without a dictionary UAX #29 splits Thai and Han
  per character, so a 4-word UAX #29 bound refuses ordinary Thai names (question
  `form-word-bound`, recommended bytes-only). NFKC, default case fold, NFKC,
  alphanumerics only: "ﬁne" -> "fine", "Ｂｒｏｋｋｒ" -> "brokkr", ZWSP and WJ drop out.

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
- Seed inventory (2026-11-10, base `5e10d7c`): features 6-8 are hedge, certainty
  and irony; 9-18 are the stance affects and restraint (slot Stance). The
  committed inventory holds revisions 1-2 only. Nothing calls `with_scene`
  except tests, and the proof packet's addressee carries no entity (its
  referent `n3` does: `entity:nibu`).
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

- Probes of 2026-11-12 at `d9ebbcf` (map pass for cut 5): no code reads
  `data/target-language-ontology/`, `data/cultural-ontology/` (since deleted) or
  `.weksa/cultural-ontology/starter-targets.cc` (a third, typed copy of en,
  ja-JP and pt-BR profiles); `README.md`, `docs/multilingual-utterance-lowering.md`,
  `examples/cultcache-lowering/` and `prompts/nibu-cultcache-target-*.md` cite
  them. `LoweringInput::with_scene` has no caller in `run.rs` or `main.rs`, the
  run pins no `SpeakerStateStore`, and no Speaker or Scene document is committed
  (`speaker-scene-seeds` open), so only the packet's `Stance` and `Discourse`
  features reach a loop's lowering view. `score::leak_diagnostic` and
  `TestReadings::at_checkpoint` take minted text as `&[&str]` and have no
  caller outside tests: no round reads glosses or any minted text against
  Validation. `data/daodejing/inventory.cc` is a revision prefix of
  `data/original/inventory.cc` (revisions 1 to 4 equal, a test proves it; revs 5 and 6 add the bed's own lexicon); with `data/interlingua/inventory.cc` they share revision 1,
  where features 6 to 8 (hedge, certainty grades, irony flag) sit in
  `Discourse` and `Predication` and features 9 to 18 (anger to restraint,
  grades) in `Stance`. `PROMPT_SOURCE_DIGEST` covers `context.rs`, `split.rs`,
  `lower.rs` and `packet.rs`. 25 `RunPaths {` literals across `run.rs`,
  `context.rs` doctests, `main.rs`, `tests/common/mod.rs`, `tests/golf_loop.rs`
  and `tests/original_corpus.rs`; 10 `Prompt::lower(` calls. `huginn known`
  timed out (Unavailable, 15 s deadline) twice this pass.

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

## Distil measure (`distil-slice`)

A lowerer is compared on round n's Validation packets with the prompts
byte-identical to the round's, by `weksa loop relower ROOT --round N --arm ARM
--lowerer LABEL [--floor ARM2 --floor ARM3 --floor ARM4]`. Every arm is answered through the one prompt
door (`RunStores::answer_files`, the Test-text guard); a tuned model is a script
that reads `<digest>.prompt` and writes `<digest>.answer` in
`<ROOT>/relower-<N>-<ARM>`, not a new adapter. The recorded reading is the
round's own lowering, read over the same items as the arm. An item the round
refused before admission (no packet) or at the lowering prompt has no prompt to
ask: it is skipped, so the readings exclude it, and it is counted as `skipped=`
on the line; `n` is the items read, so `recorded` can differ from the round's
own Validation reading, which scores those items 0. An arm answer that is not a
usable lowering scores 0 and counts in `unusable=`; the same for the floor arm,
counted (over all floor arms) after `floor ... unusable=`. The floor needs three
or more floor arms, each a prompted repeat of the recorded lowering on the same
prompts: it is `FLOOR_Z` (3) standard errors of the mean per-item difference
between the recorded lowering and a repeat, per metric, with the variances
pooled over the arms (`paired_floor`; lowering-only noise, no encoder noise). The
verdict is `decide_panel` over (recorded, arm, floor): Accepted is a composite
gain past the noise of the panel's sum (the root of the languages' composite
floors squared, at least one step), Rejected(Regressed) a loss past a language's
floor, Rejected(NoGain) neither. The cycle's own floor is the same `paired_floor`
(`floor_of`): round 1 (the recorded lowering, the head's establishing round)
against rounds 2 to `FLOOR_ARMS_MIN` + 1 (prompted repeats under the run's first
inventory), per panel language, from the rounds' per-item Validation scores. It
is measured once per run and is the same after an accept; a run needs four
`loop round` calls before its first `loop cycle`.
Calibration (`runs/distil-base`, 8 prompted draws by one model family on
identical prompts, no draw sharing more than 7 of 28 answers with another; 14
en-US and 14 ja-JP Validation items; every base, candidate and 3-arm floor
combination, 1120 trials from 28 unordered pairs; intervals bootstrap over
draws, each degraded arm one fixed draw): a prompted repeat reads NoGain 92%
(80 to 100), Regressed 8% (0 to 20), Accepted 0%, which with 28 pairs bounds
false-accept only to about 10%; an all-"Yes." arm reads Regressed 100%; a mild
degradation (one detail blurred per utterance) 100%; a subtler one 75% (50 to
100), whose mean composite drop (0.044) sits under the mean floor (0.060).
`FLOOR_Z` was chosen on this data (2.5 gives NoGain 84% and 93% on the subtler
arm), so the rates are in-sample. Without the noise term in the gain test a
prompted repeat reads Accepted in 40% of the trials.
Power (can the rule Accept a real gain?): the only better arm that could be
built without Test or Validation answers is a lowerer shown the Train sentences
of the corpus (no packets, no Validation or Test text) as a register guide.
Four such draws beat the prompted draws' mean composite in both languages (en-US
0.742 to 0.759 to 0.795; ja-JP 0.559 to 0.578 to 0.620), so the premise that it is
better is measured on the very Validation items the rule reads, not independent.
Against the prompted bases with 3-arm floors the rule reads it Accepted in 29%
of 1120 trials, NoGain 71%, Regressed 0%: a net gain of 0.079 against a mean
noise term of 0.079. At 14 items per language the rule is therefore a low-power
filter for a gain of this size; by 1/sqrt(n), about 35 or more items per
language would bring the noise term near 0.05 (an extrapolation, not measured).
### Validation growth (`validation-growth`, built; recalibration not run)

Power needs about 35 or more Validation items per language. `data/original`
now holds 35 per language (it held 14; Train 14 and Test 8 are unchanged):
42 model-written items, `item:vgrow:1..21` (en-US) and `item:vgrow-ja:1..21`
(ja-JP), live Train+Validation 56 to 98. The slug is new on purpose: the
`item:original:` prefix filter of `DIGEST_OF_THE_28` still selects the first 28
and passes; `DIGEST_OF_THE_42` freezes the new prose.

The split is derived from an item's group alone (`SplitScheme::V1`: FNV-1a of
`weksa-split-v1:<group>` mod 100, under 40 Train, under 80 Validation, else
Test). The names were chosen before any text existed: 90 candidate nouns per
language, dropped by script when the group already existed or the noun
occurred in a Test text, then the first 24 whose bucket fell in 40..=79 (the
bucket came from `SplitScheme::V1` itself, cross-checked on the 72 existing
items). Each language was written by three blind writer agents (8 nouns each,
a brief of language, register note, noun and speech act, no Test text and no
existing item), 24 texts per language written, the first 21 in name order
admitted through `weksa corpus add`; each group is its own group. The scenes
are `weksa scene put` documents (keeper to `unnamed`, inventory rev 4),
annotated by a separate agent; a second annotator measured agreement only and
is not committed.

The writer is Claude (family Anthropic recorded): no cross-family generator was
reachable from the body that ran this cut. The self-preference effect (a Claude
lowerer favouring Claude prose, Panickssery et al. 2024) is therefore not
measured. The writers' blindness is by brief content, not sandbox.

Expect encode refusals: the existing 56 were written onto the Dao r3 concepts
and the inventory was drafted from them, while the new items were written free
of the inventory (question `validation-growth-vocabulary` stays open). Some new
packets will be refused at encode (inventory coverage) and score 0 as the means
do; they stay in the corpus. That is a result about inventory generalization,
not a defect.

Every calibration number above (92/0/8 prompted rates, the 29% power, the 0.079
noise term) is for the 14-item corpus; the corpus digest changed, and the
numbers are redone on the grown corpus in a separate recalibration step.

### Smallest measurable distil slice (`distil-incontext`, not built)

Distil is a tuned lowerer measured against the prompted one; the tuned model
needs a host (follow-up `distil-host-question-owed`). The smallest slice that
measures whether Train pairs buy a gain with the rule that exists is
training-free: an in-context arm, the prompted lowerer given Train
(packet, text) pairs as few-shot examples, answered through the one prompt door
and read by `loop relower` against three prompted repeats. Parts: 12b's
Train-only export (follow-up `distil-export-at-freeze`, guarded against Test
text) as the few-shot source; the arm's directory of answers; the existing
floor and verdict. A weaker version ran on 14 items: a lowerer shown the 28
Train sentences (no packets) beat the prompted draws in both languages and the
rule Accepted it in 29% of 1120 trials. The slice is decisive only with the
Validation growth above: at 14 items it can only fail to reject. It decides the
fine-tune question in one direction: if in-context Train pairs do not clear the
floor at 35 items, a tuned model's gain over the prompted one is not
measurable with this corpus either.

Training pairs are Train only, at a frozen revision; the export is 12b.

## Lowerer first cut (`golf-loop-lowerer-cut`)

Authority map for the smallest lowerer, written before its code (Imagination
pass by the Weksa Self run, 2026-11-01). The cut exists to give the golf loop a
measurable baseline from a packet: `lower` gives text, `score` gives numbers.

| Decision | Owner | Inputs | Outputs | Not the owner |
|---|---|---|---|---|
| Whether a packet exists | `validate` / `PacketStore::admit` (unchanged) | packet, inventory, corpus, entities | admitted packet or typed refusal | the lowerer |
| Wording of a scored item in English (golf loop, `golf-loop-model-lowerer`) | the ModelPort's answer to `Prompt::lower` (`context.rs`), read as `LoweringDraft` (`encode.rs`) and held in `run::score_round`'s locals | the admitted view, the inventory at its revision, en-US entity forms | `{"text"}`, scored once, never stored | the corpus item, the scorer, the editor |
| Wording of a packet in English (`weksa lower`, proof fixtures) | `crates/weksa/src/lower.rs`: `lower(&LoweringInput, &Inventory) -> Result<String, LowerError>` | the validated lowering view and the inventory at its revision | text, or a typed refusal naming the missing template | the corpus item, the scorer, the golf loop |
| English template table (concept words, frame templates, speech-act openers) | the same module, as rows `(inventory id, gloss it speaks for, surface)` | none; it is data in code | rows the lowerer looks up | the inventory (it owns the ids and glosses, not the English) |
| The inventory's concepts and frames | an inventory revision; here `inventory::proof_revision()` (rev 2), hand-seeded as `seed()` is | none | the 6 concepts and 3 frames `nibu-proof-001` and the consent-gated request packet name | the lowerer |
| What a score counts | `Scorer::score` (unchanged) | output text, reference text | `ItemScore` | `weksa score`, which only reads text and calls it |
| The proof packet | `packet::proof_packet()`, typed ids of rev 2, admitted by `weksa packet proof <store>` through `PacketStore::admit`. Not JSON: `Packet`'s serde is CultCache's positional tuple, and JSON in is the xenos boundary the cut 7 encoder owns | none | admitted packet | the CLI decides nothing about validity |

Decisions this map makes, and why:

- **`lower` takes `&LoweringInput`, not `&Packet`.** A `Packet` carries `trace`
  and `provenance.corpus_item`; the model page says the lowerer sees the packet
  without its trace and never the corpus item (`lowering-traced`,
  `leak-owned-by-flow-and-scorer-3`). `validate` already yields the view, so the
  CLI runs `validate` and hands it over. Cut 4 narrows the door to
  `PacketStore::admit`; this cut does not wait on it.
- **The frame owns the verb phrase; a predication's concept must resolve but is
  not rendered.** Referent concepts supply nouns. A frame template names its
  roles by inventory gloss (`{agent}`), resolved against the inventory at the
  packet's revision, so a retired or re-glossed role cannot be silently
  mis-keyed. A row whose gloss differs from the inventory's entry for its id is
  a `LowerError`, so a table row cannot speak for a different meaning than its
  id now carries.
- **Rendering rules, all deterministic and all from packet fields:** the
  discourse addressee renders "you" ("yourself" when it already filled a role
  in the same predication), other referents render "the <concept word>", a
  predication filling a role renders "that", the speech-act row supplies an
  optional opener, sentences are capitalised and end in a full stop.
  Entity-bearing referents render by concept word: the lowerer does not read
  the entity store in this cut (follow-up: entity forms in the lowerer).
- **No style, no register, no features.** Hedge, certainty, modality and
  polarity are not rendered, so the baseline scores what meaning alone buys.
  A lowerer that ignores `polarity: negative` would assert the opposite, so
  `lower` refuses any packet that carries a feature, relation or extension it
  does not render rather than render it wrong (`packet-owns-meaning`); features arrive as templates
  learn them.
- **Deleted by later cuts, not this one:** the table in `lower.rs` and
  `proof_revision()` are temporary owners. Cut 5 (`language-ontology`) moves
  the table to `weksa.ontology_entry` documents selected by the target
  profile; cut 7 replaces `proof_revision()` with the loop's minting round.
  Nothing is deleted today because no earlier lowerer exists; the YAML
  `examples/english-lowering/` hand-lowerings are fixtures, not owners, and
  stay.
- **`weksa score` is a thin reader of `Scorer::score`**: `weksa score <item>...
  [--output FILE]` embeds the output text once against every named item. It
  stores nothing (Score documents arrive with cut 7) and gates nothing.

## Cut sequence

One line per cut; specs exist for the cuts about to run (2d and 4, siblings on `869c82b`; 2d merges first and 4 rebases onto it).

1. `park-node-body`: park the Node daemon, MiMo command, RUDP shims, launchers and the Discord-sourced Metame harness at tag `parked/weksa-node-body`; README front door tells the truth. Landed.
2. `packet-store`: Rust crate with packet, inventory and corpus types, the validator, the derived split, the stores and the `weksa` CLI. Landed at `28284cc`.
2b. Landed. `packet-store-corpus`: corpus labels tell the truth (`corpus-labels-tell-truth`): `ModelFamily` and the generator string on `ModelWritten`; `item:nibu:1` ModelWritten (Unrecorded); `item:brokkr:1` and its fixture carry `codex-local-source-aware-draft`, family OpenAi; intake refuses aliased keys, a whole item changed under its key, and a held item's text under a new key; corpus.cc reseeded. Speaker state is not in 2b: it lives in Speaker and Scene documents and the packet's stance, not on the item. Waits on 2. (spec)
2c. Landed. `entity-store`: `ProperName` and `Referent.name` deleted; `EntityId`, `Referent.entity`, the `weksa.entity` document and its store; corpus name spans and entity-id speakers; the packet store's entity checks; seed entities and spans for the cast items; schema v2. Waits on 2b. (spec r1)
3. Landed at `869c82b`. `scorer`: chrF, meaning cosine behind an `Embedder` port (Ollama adapter with `num_thread`), leak diagnostics, aggregation by derived split (Train and Validation only on the round path; Test only through `TestReadings::at_checkpoint`, which consumes a `Checkpoint`); library only, `weksa score` comes with the loop. h2 landed the r2 fix pass on its branch; r3 deletes the name mask (the scorer reads output and reference only) and pools novel coverage per order over word keys that split joined labels. Waits on 2c for merge order. (spec r3)
2d. `entity-door-lows`: invisible format characters refused in entity forms (ZWJ, ZWNJ, CGJ and variation selectors kept); the form length bound per question `form-word-bound` (recommended: bytes only); attested origins (`Cast { persona, fixture }`, `Live { persona }`) and `CastAuthored.author`; empty provenance strings refused at the store; S6 under NFKC plus case fold and no-alphanumeric texts refused; `next_key` refuses at `u32::MAX`; CLI flags that do not apply refuse and no error echoes a value. Owns follow-ups `entity-door-lows` and `s6-unicode-normal-form`. (spec r1)
3b. `lowerer-first-cut` (follow-up `golf-loop-lowerer-cut`): the section above. `lower`, `weksa lower`, `weksa score`, `weksa packet proof`, inventory rev 2 for `nibu-proof-001`.
4. `speaker-state`: packet `stance` (felt affect and restraint, slot `Stance`, packet schema v4); formality, warmth and intensity out of the seed and the standard; only `PacketStore::admit` yields a `LoweringInput`, and it refuses packets for withdrawn items (follow-up `lowering-input-from-packet-store`). (spec r1)
4a. `speaker-relation-input`: slot `Scene` and its first seed features (`relative_status`, `intimacy`, `in_group`; inventory revision 3, now `speaker_scene_revision`); `LoweringInput::with_scene` carries the speaker's relation to the addressee as typed `Scene` feature values. Since 4b it takes the `SpeakerStateStore` and the `CorpusStore`, reads the scene of the view's own item group for the item's speaker, and selects the relation by the addressee referent's entity (V16 when the scene does not list the item's speaker or the speaker of any live item of the group, the rule `put_scene` applies too; V17 refuses an addressee that names a predication; V13 is retired). The English lowerer does not render it. (PR for follow-up `chat-s3352-3352`)
4b. `speaker-scene` (spec r1; `speaker-scene-seeds` is its cut 4c: CLI, inventory revision 3, cast speakers and scenes; `Party::Unnamed` covers an addressee with no entity; Speaker and Scene share one `SpeakerStateStore`): slots `Speaker` and `Scene` with their seed features; `weksa.speaker` (`speaker:<entity-slug>@<n>`) and `weksa.scene` (`scene:<group>@<n>`, participants and ordered-pair relations) documents with `Annotator` `Operator` and `Model { family, generator }`, validated by the packet's shared feature check, append-only per key; their stores, `weksa speaker` and `weksa scene` commands, published schemas; Model-annotated cast speakers and scenes for the committed items. Waits on 2d and 4; `pd-speaker-annotation` blocks only the public-domain annotator. Part 1 landed (branch `weksa-lowering/speaker-scene`): `speaker.rs`, `SpeakerStateStore`, slots `Speaker` and `Scene` exclusive to their own feature lists, `speaker_scene_revision()` (Speaker features 22-24), `with_scene` on the store, schemas. The CLI, the committed inventory revision 3 and the seeds stay with 4c.
5. `language-ontology`, in sequential cuts on `d9ebbcf`. 5a `language-ontology` (spec r1): `weksa.ontology_entry`, `OntologyStore`, `ontology::select` over the view's `Stance` and `Discourse` features, `Prompt::lower` printing the selected entries of the item's language, `RunRecord` pinning the ontology store (keys 15 and 16), `weksa ontology admit|dump`; `ontology.rs` joins `PROMPT_SOURCE_DIGEST`. 5b `language-ontology-seed` (spec r1, after 5a): Model-annotated en-US and ja-JP seed entries drafted by an isolated model, `data/target-language-ontology/`, `.weksa/cultural-ontology/` and `schemas/target-language-cultural-ontology.md` deleted. Lost until 5c': keigo, hierarchy and uchi-soto hang on the scene relation from 5c', so ja-JP politeness is still the lowerer's guess. 5c' `language-ontology-scene` (spec r1, after 5b): the run pins the `SpeakerStateStore` (keys 17 and 18, `Drift(SpeakerState)`), `RunStores::assembled` is the one door that reads the item's scene into a loop view, `select` reads slot `Scene`, `Prompt::lower` refuses a view whose scene was never read, `weksa scene put|dump` and `weksa inventory speaker-scene` (rev 4 in data/original and data/daodejing, rev 3 in data/interlingua, which holds no Dao revision), Model-annotated keeper scenes and Scene-conditioned entries for `data/original`. 5c `language-ontology-view-assembly` (follow-up, now only the Speaker read): `select` widens to slot `Speaker`; waits on `speaker-scene-seeds`. 5d `language-ontology-communities` (follow-up): `weksa.community` built fresh (`data/cultural-ontology/` is deleted by 5e), community ids on `weksa.speaker`, community conditions; waits on 5c and question `persona-speech-community-home`.
5e. `delete-cultural-ontology` (steward ruling, 2026-11-13): `data/cultural-ontology/` (five draft YAML profile files and a README; `old-norse-edda-source-tradition.yaml` did not parse) deleted. Nothing in `crates/`, tests, scripts or CI read it; `examples/persona-lowering/brokkr-edda-voice/` cites its paths as text only (the one test that reads that example reads `lowered-output.yaml`). The typed `weksa.community` of follow-up `language-ontology-communities` is built fresh under cut 5; follow-up `edda-referent-yaml-parse` is moot.
6. `pd-importer`: public-domain play and fiction dialogue into the corpus with speaker, scene group, edition provenance, and Speaker and Scene annotations by the ruled annotator. Waits on 4, follow-up `gutenberg-terms` and question `pd-speaker-annotation`.
7a. `loop-context-builder`: `context.rs` (`TrainItems`, `EncoderOffer`, `Prompt`: corpus text reaches a model call only through `Prompt::mint` over live Train items or `Prompt::encode` over one offered item; instructions are the consts `MINT_INSTRUCTIONS` and `ENCODE_INSTRUCTIONS` in `context.rs`; `Checkpoint` has no public constructor, offers each item once, and its offers borrow it (the gate is typed `TestReadings` and Test offers on the round path; operator tools such as `weksa corpus dump` and `weksa score` still read corpus text of any split); prior turns print the held Train text), `SplitScheme::live`, `CorpusItem::cited_entities` (one owner of the id set `PacketStore::admit` and the offer share), and checkpoint-only Test readings (`Checkpoint`, `TestReadings`; `Aggregate` and `leak_diagnostic` carry no Test). Library only; since 7b `TrainItems::from_corpus` and `EncoderOffer::for_round` are crate-private and `RunStores` is their one public producer. Merges follow-ups `loop-context-builder-2` and `scorer-test-readings-type`. (spec r1)
7b. `golf-loop-round` (spec r1; branch `weksa-lowering/golf-loop-round`): the smallest loop that scores one round. `run.rs`: `RunStores` pins the corpus and entity stores of a run by canonical path and FNV-1a 64 digest at `create`, reopens only from `<root>/run.cc` and refuses drift (corpus, entities, `PROMPT_SOURCE_DIGEST` of `context.rs`, a second pin on `lower.rs` retired in 7d, model, embedder); `round` encodes every live Train and Validation item through a `ModelPort` (`model.rs`; the one adapter `AnswerFiles` writes `<digest>.prompt` and reads `<digest>.answer`, so the model is the Claude session), reads each answer into a `PacketDraft` (`encode.rs`, the one place a model's JSON is read), admits through `PacketStore::admit` into `<root>/round-<n>/packets.cc`, lowers, scores once, and keeps a `RoundRecord` (`weksa.loop_round`, no text, no Test; Train rows, Validation only as means and refusal counts). `weksa loop open|round`. Not here: the spec-edit step, accept rule and composite, noise floor, Score and lowering documents, a subprocess adapter, the checkpoint door, a model lowerer (follow-ups `golf-loop-spec-edit`, `golf-loop-accept-rule`, `golf-loop-score-documents`, `golf-loop-subprocess-adapter`, `golf-loop-checkpoint-door-bound`, `golf-loop-model-lowerer`). `packet-store-inventory-binding` is met for loop packets only (`RunStores` admits with the run's inventory); `validate`, `packet proof|dao` and `lower` still take caller stores. Waits on 7a, `checkpoint-gate`, 4b.
7c. `golf-loop-edit-accept` (spec r1; branch `weksa-lowering/golf-loop-edit-accept`): one edit-accept cycle. The run works on its own copy of the inventory (`create` copies the caller's file to `<root>/inventory.cc`, refusing a root that already holds a file of that name, and pins its digest as `RunRecord` key 11; `RunRecord.inventory` is provenance; a `run.cc` from before has no key 11 and does not open); the head is derived from the latest Accepted `Decision`. `weksa loop cycle` asks the editor for one inventory edit from `Prompt::mint(train, inventory, rev, feedback)` (Train blocks, the inventory listing and an `EditFeedback` line: whether the last edit was kept and the head's Validation composite rounded to `LADDER_STEP`, the Ladder release of Blum and Hardt 2015), reads it as an `EditDraft` (`encode.rs`; ids and revisions are the store's, `into_revision`), admits it through `InventoryStore::admit_revision` into `<root>/cycle-<k>/inventory.cc`, runs a candidate round under it, and `decide`s on Validation against the head's establishing round past a floor (never under one `LADDER_STEP`; from cut `cycle-floor-paired` the paired floor over rounds 1 to `FLOOR_ARMS_MIN` + 1, so a cycle needs four recorded rounds, where 7c measured the spread between rounds 1 and 2). An edit that fails is a rejected `Decision`, not an error. `RoundRecord` gains keys 16 (inventory digest) and 17 (cycle); an encode answer that is not UTF-8 is that item's parse refusal. Not here: Score and lowering documents (`golf-loop-score-docs`), Train rows in the edit prompt (`golf-loop-edit-train-feedback`), a paired floor (`golf-loop-paired-floor`), the checkpoint door and merging to `data/` (`golf-loop-checkpoint-door-bound`, `golf-loop-freeze-merge`), a model lowerer (`golf-loop-model-lowerer-next`; until it lands an entry an edit adds has no lowering row, so a cycle gains only by retiring entries or adding ones the encoder does not cite). Waits on 7b.
7d. `golf-loop-model-lowerer` (spec r1; branch `weksa-lowering/golf-loop-model-lowerer`): the round lowers through the `ModelPort`. `Prompt::lower` (`context.rs`) takes a `LoweringInput` (no text, key, group or speaker getter), the inventory at the view's revision and the en-US entity forms, and prints `LOWER_INSTRUCTIONS`, the meaning of the packet, the entities line and the inventory listing; the item key goes only to `item_keys`. The answer is `{"text"}`, read as `LoweringDraft` in `encode.rs`, held in `score_round`'s locals and scored once; it is never put in `run.cc`, a `RoundRecord`, a `Decision` or `EditFeedback`. A round has two answer phases (every encode prompt, then every lowering prompt), all or nothing: while any prompt is unanswered `packets.cc` is discarded and nothing reaches `run.cc`. `lower::lower` and the template table leave the loop (they remain `weksa lower`'s; follow-up `table-retirement`), so an entry an edit adds can be said and a cycle can buy a gain. One source pin remains: `PROMPT_SOURCE_DIGEST` covers `context.rs`, `split.rs` and `lower.rs` (`RunRecord` key 7 and `RoundRecord` key 3 are retired). The run's record pins `LowererReach` (`weksa loop open --lowerer-reach briefed-subagent|tool-less-relay`, `RunRecord` key 12) and every printed round and cycle line says it; a briefed-subagent run says its numbers are smoke-round only (rulings `lowerer-call-reach-toolless-relay`, `dao-round-seed-inventory`). A lowering is refused at `Lower` when blank, over `MAX_LOWERING_BYTES` (4096) or holding a control character other than a line break or tab; `PROMPT_SOURCE_DIGEST` also covers `packet.rs`. Not here: per-item lowering text documents (`golf-loop-score-docs`), the subprocess adapter. Waits on 7c.
7. `golf-loop-en` (7b and 7c landed the round, `ModelPort`, encoder prompt, template digest, round records and the edit-accept cycle; the rest remains): the packet store bound to one inventory store everywhere (`packet-store-inventory-binding`), a subprocess adapter, Score documents, `weksa score` in the loop, the detached unit on Yggdrasil. Waits on 3, 5, 6 and follow-up `claude-on-yggdrasil`.
8. `style-axis`: a style-embedding score behind the scorer port. Waits on 3.
9. `ja-panel`: ja-JP into the loop's panel, in two cuts on `e9cbd9b`. 9a `ja-panel-loop` (spec r1): language is the item's and the panel the run's: `Prompt::lower` takes the item's language and prints it on a language line (no prompt names English), entity forms are read in that language, `RunRecord` pins the panel (`weksa loop open --panel`), offers, mint and recall read only panel items, round means, floors and readings are per panel language, the accept rule runs `decide` per language, the recall split works on unspaced scripts, and `weksa corpus add` has no default language. 9b `ja-panel-corpus` (spec r1, after 9a): the ja-JP slice of `data/original`, model-written Japanese Train and Validation passages and eight Aozora Bunko public-domain Test sentences with full provenance, and their entities. Cut 5 is not a prerequisite: neither language lowers with ontology entries until cut 5 gives both theirs.
10. `conlang-lexicon`: the first conlang's lexicon placed at concept embeddings, its morphology data, its ontology entries and the gloss reader. Waits on 3, 5 and question `first-conlang`.
11. `convergence-freeze`: the convergence test, `Checkpoint`'s one door (built from the convergence window over the persisted round records), the freeze, the re-encode into training pairs. Waits on 7, 9 and 10.
12. `distil`, three cuts. 12a `distil-slice` (relower and the prompted baseline; spec r1). 12b `distil-export`: Train-only prompt/completion JSONL at a frozen revision, Test-text guarded (follow-up `distil-export-at-freeze`); waits on 11. 12c `distil-train`: waits on 12b and the host question (follow-up `distil-host-question-owed`).
13. `live-lowering`: agents emit packets with stance; Weksa reads speaker state from persona state (follow-up `persona-state-speech-community`) and lowers for the reader. Waits on 11.
