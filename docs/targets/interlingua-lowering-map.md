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
(`weksa-lowering:ruling:leak-owned-by-flow-and-scorer`). The golf loop improves the spec
mostly by growing and refining the inventory, which is fast; schema changes are
the slow path. Concepts are open-class and grow forever; the other families are
closed-class, and convergence is measured on them
(`weksa-lowering:ruling:convergence-test`). This split was
`spec-is-schema-plus-inventory`, now carried by `leak-owned-by-flow-and-scorer`.

## Model page

Every persistent kind, what names it, its life, and who decides. "Derived"
means no store holds it and nothing writes it.

| Kind | What names it | Lifecycle | Who decides |
|---|---|---|---|
| Packet | Key `packet:<16 hex>`, an opaque digest of item, inventory revision and encoder (the item key lives in `provenance.corpus_item`, which the lowerer never sees); CultCache type `weksa.interlingua_packet`, carrying `schema_version` and `inventory_rev`. Every id inside is an opaque form type (`n<k>` nodes, `encoder:<16 hex>`, canonical `<family>:<n>`) | Minted by an encoder for one corpus item under one spec version; immutable; a new spec version means a new packet, never an edit. Admitted only through `PacketStore::admit`, which validates against the inventory store at the packet's revision | The encoder writes; the form types refuse malformed ids at construction and load, the validator refuses broken references and over-capacity packets: a feature at most once per slot, at most 64 nodes (`typed-packet`); nothing else writes packets |
| Schema version | `weksa.interlingua_packet.v<N>` constant in `crates/weksa`; published JSON Schema at `schemas/cultnet/` | Bumped by a commit that changes the Rust types; the published schema is regenerated in the same commit and a test fails on drift | A cut (Hands, under a spec); never the loop |
| Inventory entry | Key `<family>:<n>`, `n` canonical decimal >= 1 (families: concept, frame, role, relation, speech_act, feature, referent_kind, extension); type `weksa.inventory_entry`, with a bounded gloss, value kind (option labels bounded), `introduced_rev` and optional `retired_rev` | Added or retired by an inventory revision; never edited in place (a changed meaning is a retire plus an add); a retired id still resolves for packets of earlier revisions | The loop's accepted rounds, whose minting call the context builder assembles from Train items only; Self on merge to main. Admission checks form, not text |
| Inventory revision | Key `rev:<n>`, type `weksa.inventory_revision`, with parent, change list and `frozen` | Appended per accepted round; exactly one revision is ever marked frozen, by the convergence test | The loop's accept rule appends; the convergence test freezes; Self records the freeze as a ruling |
| Target profile | Locale or language-project id (`en-US`, `ja-JP`, a conlang's slug) plus a revision | Authored, revised by commit or by an accepted round that changed the lowering; each lowering names the revision it used | Profile authors and accepted rounds (`profile-owns-register`); never the lowerer's defaults |
| Corpus item | Key `item:<source-slug>:<n>`; type `weksa.corpus_item` with text, language, speaker, split group, prior turns, source | Admitted once; the text is immutable (a correction is a new item that `supersedes` the old); withdrawn, never deleted, so old scores stay explainable | Importers and `weksa corpus add`, under `corpus-cast-plus-public-domain`; the source enum has no variant for Discord or any training-forbidden source (`corpus-clean`) |
| Split assignment | Derived: `split(scheme, item.group)` | Fixed for a scheme; a new scheme is a new constant, never a reassignment. A model-written item whose group derives test is in no split; a superseding item keeps its predecessor's group | The hash function. Nobody assigns by hand, so there is no writer to forbid |
| Loop round | Key `run:<label>:round:<n>`; type `weksa.loop_round` with spec version, profile revisions, model ids and prompt digests, sampled train items, proposed change, validation scores before and after, outcome | Opened, scored, closed accepted or rejected; immutable once closed. Lives in the run store on Yggdrasil | The loop driver's accept rule (validation composite improves past the measured noise floor and no metric regresses past tolerance) |
| Score | Key `<lowering-key>:<metric>@<metric-version>`; type `weksa.score` | Immutable; a new metric version scores again under a new key | The scorer, deterministic given the metric version and the embedder's model id |
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

## Scoring (`golf-metric-automatic`, `conlang-scoring`, `meaning-metric-embedding-cosine`)

- English against its corpus item: chrF (character n-gram F-score, beta 2; a
  20-line implementation matched sacrebleu 2.4.3 exactly on four probe pairs)
  and meaning cosine (sentence embeddings from qwen3-embedding:0.6b, already
  served by Ollama on Yggdrasil). Probe on one reference: hedged paraphrase
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
- Leak (`leak-owned-by-flow-and-scorer`): two owners, no door. The loop's
  context builder gives inventory-minting calls Train items only, the encoder
  emits only a packet, and prompts are fixed within a run. The scorer masks
  every token of the packet's referent names from reference and output before
  chrF and cosine, and reports, beside the scores and not as gates, chrF of
  minted glosses and labels against Validation and Test and the share the mask
  removed. Rejected path: text rules at the inventory and packet doors
  (per-entry 3-gram, name case). Soul s2 beat them with chunked glosses,
  hyphenated or caseless names and an undeclared `minted_for`, and a door
  cannot know what a writer saw.

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

One line per cut; only the first two have specs.

1. `park-node-body`: park the Node daemon, MiMo command, RUDP shims, launchers and the Discord-sourced Metame harness at tag `parked/weksa-node-body`; README front door tells the truth. Waits on nothing. (spec)
2. `packet-store`: Rust crate with packet, inventory and corpus types, the validator, the derived split, the stores and the `weksa` CLI; the standard reconciled to the types. Waits on nothing; sibling of 1. (spec r3: the s2 fix pass, deleting the door's leak rules and adding capacity rules V11, V12)
2b. `packet-store-corpus`: corpus labels tell the truth. `Source::ModelWritten { family: ModelFamily, generator: Option<String>, prompt_digest: Option<String> }`, `ModelFamily` an enum of vendor families (Anthropic, OpenAi, Google, Alibaba, Meta, Mistral, Unrecorded) so the scorer can refuse a model-backed metric of the item's own family (Unrecorded matches every family); `item:nibu:1` ModelWritten (Unrecorded) until the operator attests it; `item:brokkr:1` and its fixture carry the true generator `codex-local-source-aware-draft`, family OpenAi; corpus.cc reseeded; the codex/openai negative grep narrowed to code. Waits on 2 and a Self ruling recording the label decision.
3. `scorer`: chrF, meaning cosine behind an `Embedder` port (Ollama on Yggdrasil), the name mask and leak diagnostics (follow-up `scorer-name-mask-and-leak-diagnostics`), `weksa score`. Waits on 2.
4. `pd-importer`: public-domain play and fiction dialogue into the corpus with speaker, scene group and edition provenance. Waits on 2 and follow-up `gutenberg-terms`.
5. `golf-loop-en`: the loop driver, the Train-only context builder (follow-up `loop-context-builder-train-only`), the packet store bound to one inventory store (`packet-store-inventory-binding`), `ModelPort` with the `claude -p` adapter, encoder and lowerer prompts, round records, the accept rule, the detached unit on Yggdrasil. Waits on 3, 4 and follow-up `claude-on-yggdrasil`.
6. `style-axis`: a style-embedding score behind the scorer port. Waits on 3.
7. `ja-panel`: ja-JP into the loop's panel. Waits on 5.
8. `conlang-lexicon`: the first conlang's lexicon placed at concept embeddings, its morphology data and the gloss reader. Waits on 3 and question `first-conlang`.
9. `convergence-freeze`: the convergence test, the freeze, the re-encode into training pairs. Waits on 5, 7 and 8.
10. `distil`: fine-tune a lowerer on the frozen training set. Waits on 9 and a host question asked then.
11. `live-lowering`: agents emit packets and Weksa lowers them for the reader. Waits on 9.
