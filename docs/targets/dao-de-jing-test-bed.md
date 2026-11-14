# Dao De Jing test bed

A lowering test bed on James Legge's public-domain translation of the Dao De
Jing, from Meta's analogy: the Dao has many varied human translations, so it is
a ready-made field of precedents for what one meaning can be lowered to. Meta
proposed it in #weksa:gamecult.org on 2026-11-06 (Matrix event
`$2_P0mbtbnnsP4I61UhVMlatuDSTjFXMOuGa4SSbhGmc`: public-domain translations only
as the scoring corpus, copyrighted ones cited without copying) and noted the
ontology consequence (event `$E4VHwS-zFEk9Fa-daFX8MkfoySn0iX0m1grltEJ9A8Y`:
model an imperial native term as a term with several context-tagged readings
plus a separately recorded chosen gloss per lowering).

## What is here

- `data/daodejing/`: its own stores (inventory revisions 1 to 3, one entity, five
  corpus items, five packets), apart from the production stores in `data/`.
  Revision 3 is `weksa::daodejing::revision()`; the packets are
  `weksa::daodejing::packets()`. The scene revision (`speaker_scene_revision()`, also
  numbered 3) is not applied in these stores and will need renumbering when it
  is applied anywhere both meet.
- Rebuild: `weksa --corpus C --inventory I --entities E` with `inventory seed`,
  `inventory proof`, `inventory dao`, `entity put entity:laozi ...`,
  `corpus add ... --language en-US --source public-domain` per item, then `packet dao <store>`.
- Source: Project Gutenberg eBook 216, "The Tao Teh King, or the Tao and its
  Characteristics", Laozi, tr. James Legge (Sacred Books of the East 39, 1891).
  Gutenberg's licence (1.C) does not claim a right to prevent derivative works of
  a work not protected by US copyright, for a user in the US, provided every
  Project Gutenberg reference is removed (see
  `docs/research/2026-11-13-gutenberg-terms.md`). The items hold only Legge's words, one numbered section
  or sentence each; no Gutenberg header, licence text or trademark is stored,
  and the eBook file itself is not committed. `edition_url` names the page the
  text was retrieved from, as a citation of provenance and not as part of any
  item's text. The legal-root question of whether a citation counts as a
  "reference" is not ruled; if Gutenberg says it does, the field moves to the
  Sacred Books of the East print edition.
- `Source::PublicDomain` now carries `licence`, `jurisdiction`, `translator` and
  `edition_year`, and the store refuses an item missing any of them (an empty
  string, or year 0).

## The slice

Corpus groups are chapters, so the derived split spreads the slice:

| item | Legge | split |
|---|---|---|
| `item:daodejing:1` | ch. 3 s. 2 | Train |
| `item:daodejing:2` | ch. 7 s. 1, first sentence | Validation |
| `item:daodejing:3` | ch. 8 s. 1, first sentence | Validation |
| `item:daodejing:4` | ch. 78 s. 4 | Test |
| `item:daodejing:5` | ch. 5 s. 2, first sentence | Test |

Each packet states neutral facts (referents, predications, a speech act) and a
stance (restraint 800 on all; affection 300 on the water likeness). The stance is
my reading of the sage's voice, not data.

## First closeness number

Scored with `weksa score` (chrF, and meaning as cosine under `qwen3-embedding:0.6b`
through the local Ollama), one lowered output against its own item.

| item | split | chrF | meaning | lowered |
|---|---|---|---|---|
| 1 | Train | 44.18 | 0.831 | The sage empties the minds. The sage fills the bellies. The sage weakens the wills. The sage strengthens the bones. |
| 2 | Validation | 82.12 | 0.976 | The heaven is long-enduring. The earth continues long. |
| 3 | Validation | 74.99 | 0.974 | The highest excellence is like the water. |
| 4 | Test | 82.50 | 0.949 | The words that are strictly true seem paradoxical. |
| 5 | Test | 75.99 | 0.894 | The space between heaven and earth may be compared to the bellows. |

Mean over the five: chrF 71.96, meaning 0.925. Mismatched baseline (each output
against the other four references, 20 pairs): chrF 14.1, meaning 0.458. So the
scorer separates the right reference from a wrong one by a wide margin; that is
all it shows here.

**Do not read 72 as a measure of the interlingua.** The five outputs are the
ceiling of an author who saw the reference:

- One author wrote the packets, the inventory entries and the lowerer rows with
  Legge's sentence open. Frame 3 (`act`) was written from the Train item. Frames
  4 and 5 came from Validation items and frames 6 and 7 from Test items, each
  written while reading that item. The Train item, the only one lowered by a
  frame minted from Train, scored lowest (chrF 44.18). The scores rise with
  how bespoke the frame is, the reverse of generalisation. The Validation and
  Test numbers measure the lowerer table, not what the interlingua can carry.
  That table is deleted: the Dao numbers below were measured on it, and the
  loop now lowers through the model (`Prompt::lower`).
- n = 5, one translation as reference, one embedding model, no variance.

## What the scorer cannot see

- **Stance.** The stance features are in the packets and the lowerer does not
  render them, so the number is blind to the packet's voice entirely. It would
  not change if every stance were deleted.
- **Dropped meaning.** Item 5 is a negated rhetorical question ("May not ... be
  compared"); the lowering is a plain statement with no question mark and no
  "not". Item 1 loses "Therefore", "in the exercise of his government" and "their".
  Item 4 loses "to be". chrF scores the overlap and cannot say that the speech
  act changed. The embedding cosine of 0.894 for item 5 is not much lower than
  for the statement-for-statement items.
- **Table defects it forgives.** "The heaven" and "the water" are wrong English
  (the lowerer prefixes "the" to every referent and has no proper-noun,
  mass-noun or entity rendering). chrF takes a small penalty only.
- **Register and meaning of the Chinese.** The packets were read off Legge's
  English, not the Chinese, so this is a round trip English to interlingua to
  English. It measures how well the packet kept Legge's wording, not whether Legge
  or the packet is right about the Dao. The term "Tao" never appears in the slice.
  The multi-reading term modelling Meta described is untested.
- **Leak.** `weksa score` scores the whole output against the whole reference
  with no mask, as the leak ruling says. Nothing here enforces that the
  author did not use the reference; only the labels above say so.

## What the run showed the interlingua needs

- Negation and question force: `polarity` is refused by the lowerer ("features")
  and `ask` has no punctuation row, so two of Laozi's habitual moves, the negated
  claim and the rhetorical question, cannot be lowered at all.
- A frame per sentence shape is the lowerer's cost model today: each new shape
  is one more table row and one more inventory frame. The Dao's aphorisms are
  mostly copular and paradoxical, which suggests the next frames to mint
  (attribution, contrast, negated identity) before more chapters are added.
