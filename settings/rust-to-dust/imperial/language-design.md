# Rust to Dust Imperial Language: Design Spec

Status: **DRAFT r0.** First conlang project for Rust to Dust. Nothing here is
canon until the operator rules on it.

The language has no name. Its working slug is `rtd-imperial`. A name is owed
and should come from the ontology and phonology below, as
[setting.md](../setting.md) asks, not from an English label. Nothing here
assigns one.

## How To Read This

Every claim carries a tag:

- **[E]** evidence. The source material says it, and the file is cited.
- **[I]** invention. This draft proposes it. The operator can overrule it, and
  it is the part most worth attacking.
- **[E→I]** an extension. The source gives a seed and this draft develops it.

Sources read: everything under `settings/rust-to-dust/` (`README`, `setting`,
`aliens`, `story`, `battle-royale`, `language-design`), the house method in
`settings/eusocial-interbeing/language-design.md`, and Weksa's
`docs/language-model.md`, `interlingua-standard.md`,
`best-modern-approach.md`, `multilingual-utterance-lowering.md` and
`schemas/interlingua-packet.md`. The stance design is read from `main` (see Dependencies). The forge repos and
Weksa `state/` and `docs/` hold no other Rust to Dust material: a forge search
for the setting returned nothing outside Weksa. This draft therefore rests on
about 440 lines of setting prose, and the invention share is large.

## 1. Speech Community

### Who speaks it [E]

The empire's aliens: reptilian, scale-decorating, aesthetics-as-moral-truth,
honor as devotion to aesthetic perfection in a chosen field (`aliens.md`). The
empire is galaxy-spanning, so the language is a **network language**. Its
community is defined by the imperial cultural network (media, prestige,
commerce), not by a homeland. Humans are not native speakers. They meet it
through subtitles, memes and broadcasts, and their contaminated English is a
separate register (`language-design.md`, Registers).

### Speech situations [E→I]

`language-design.md` lists registers. This draft turns each into a situation
with a speaker, an addressee and a stake, because a register is a relationship
and not a vocabulary list:

| Situation | Speaker → addressee | Stake | Source |
|---|---|---|---|
| Broadcast commentary | commentator → mass imperial audience, about a performer | rendering an aesthetic verdict in real time | E (battle-royale) |
| High honor speech | warrior or master → rival, patron or witness | binding a vow, naming a performance | E (register list) |
| Warrior discourse | practitioner ↔ practitioner of one field | form, discipline, challenge | E (register list) |
| Commercial acquisition | buyer ↔ seller, or buyer → cultural object | access and integration phrased as uplift | E (setting: commerce as delivery rail) |
| Panel-show media | host ↔ guest before an audience | turning the guest's stance into performance | E (story: the panel) |
| Tourist admiration | tourist → anything | consuming a safe spectacle | E (aliens: tourists) |
| Address to humans | any → human | uplift, fascination, appraisal | I |

The last row is added. The story's central irony is that the empire
understands the father's refusal better than he understands the empire, so
how imperial speakers address humans is a design surface, not an afterthought.

The first proving slice is **broadcast commentary** [E]: `battle-royale.md`
names it as the runtime use, and it needs the fewest situations to be
distinguishable by a reader.

### Status ladder [I]

The source gives status *signals* (scale decoration marks profession, class,
rank, grief, celebrity, courtship, school, combat discipline, public
obligation, commitments) and a *measure* (the more elaborate the decoration,
the more visible the claim of virtue). It gives no ladder. The invention:
status is two axes, not one.

- **Field.** The discipline one is devoted to (combat, speech, hosting, trade,
  politics, art). Honor is field-relative [E]. A master of one field has no
  rank in another.
- **Standing in the field**, by *claim and demonstrated constraint*, not by
  force:
  1. **Unclaimed.** No decoration in a field. This is shameless, not neutral
     [E: unpainted = shamelessness].
  2. **Claimant.** Decorated, undemonstrated. Tourists live here: the claim is
     bought and the constraint never chosen [E: tourists buy a safe spectacle].
  3. **Practitioner.** Decorated, with constraint demonstrated in the field.
  4. **Master.** Constraint demonstrated before witnesses.
  5. **Exemplar.** Master whose form others cite.
- **Bare-by-claim** is an exceptional state off the ladder, not a step on it.
  Going unpainted as an artistic claim that the performance alone is art is a
  provocation [E]. The grammar must be able to say it without saying
  "shameless".

The language's address and agreement system tracks **claim versus
demonstrated constraint**, because that is what the source treats as the
moral measure. A tourist's loud, expensive decoration is a *mismatch* the
language can state, and the language carries that mismatch in the grammar
itself.

Decoration is the primary status channel in the world. In speech it surfaces
as address terms, honorific-like marking and metaphor (Section 4), not as a
phoneme-level trick.

## 2. Phonology (provisional)

The source gives no phonetic profile. It poses the question
(`language-design.md`, Open Questions) and says naming should follow from
phonetics. House method (Zyphos) assigns Airawa hard dorsal and coronal
edges and Sa'ueia soft flow. This language must not collapse onto either
[I].

### Design stance [I]

Three pressures from the source, turned into sound:

1. **Aesthetic devotion**: the language should sound *composed*. Prosody is
   deliberate, with controlled length, no clutter.
2. **Chosen constraint is mastery**: constraint should be audible. Phonotactics
   are *tight*, so that a speaker visibly refrains from the permitted-in-vulgar
   alternatives.
3. **Reptilian body** [E: reptilian]: the inventory leans on sustained
   fricatives, hisses and trills that a non-human vocal tract makes easy. The
   source does not specify a vocal tract. This is the weakest-evidenced
   choice and a human-pronounceable romanization is therefore required.

### Inventory (draft)

- **Stops** (voiceless only): `t`, `k`, `ʔ`. No voiced stops. This separates it
  from Airawa's `g d q`.
- **Fricatives**: `s`, `ʃ`, `x`, `h`, with voiced `z`, `ʒ`. Fricatives
  contrast in **length**: short and held. Held fricatives are the language's
  main prosodic ornament and carry register weight (Section 3).
- **Sonorants**: `n`, `m`, `l`, trilled `r`.
- **Vowels**: `a`, `i`, `u`, `e`, each short or long. A long vowel is the
  other register-bearing length.
- **Syllables**: `(C)V(C)`, coda `s n l ʔ` only. No clusters. The absence of
  clusters is the phonotactic form of "chosen constraint".

### Prosody [I]

- Stress and length are the speaker's expressive budget. In formal
  registers, *less* length variation reads as more command of the form. A
  commentator in broadcast register may spend length on a performer's
  moment, and an honor-speech speaker withholds it. Restraint is therefore
  audible without a phoneme change (Section 3).
- Broadcast commentary has a lengthened, sibilant "announcer" delivery. Tourists
  over-lengthen. Master speech under-lengthens.

### Romanization and rendering [I]

An ASCII romanization is owed before any lowering output is scored: the
lowering renders text, and the renderer for the audience needs
a stable orthography. `ʃ ʒ x ʔ` need ASCII digraphs. The choice follows
the Weksa pronunciation-lowering docs and belongs to a later cut.

### Open

- The inventory is a sketch, drawn from three pressures. It should be rerun
  after at least a hundred roots exist, not defended.
- The source calls for names to "emerge from ontology and phonetics". This
  draft supplies phonetics, and the name waits for the first root set.

## 3. Register, Status and Restraint: Stance Explicit

### Weksa's constraint [E]

On `main`, `docs/interlingua-standard.md` carries
a packet `stance`: **felt affect** (anger, fear, sadness, joy, disgust, surprise,
contempt, affection, shame, as grades) and **`restraint`**, how much of the
felt affect is held back (0 none, 1000 fully). It is felt, never displayed.
How a feeling shows and how much leaks is cultural; each language's ontology
maps felt affect and restraint to its own register. A held-back feeling is not
a claim, so it is never a predication.

This language must therefore **mark stance explicitly in its own grammar**,
and the marking is derived from the packet's `stance`, never from English
register words in the source text.

### Why this language is the hard case for restraint [E→I]

For the empire, restraint is not suppression of a shameful interior. It is
**the visible form of chosen constraint**, which is the moral proof of
mastery [E]. So the map from `restraint` to surface is *inverted* relative to
most human defaults:

- High restraint is the *display* of virtue. A fully restrained speaker with
  strong felt affect is exhibiting form.
- Low restraint with strong affect is not honest. It is *vulgar* [E: "vulgar
  violence as failed form"; "collapse into mere function"].
- Zero affect is not neutrality. It can read as an absence of devotion.

The deliberate consequence is that stance cannot be flattened to a politeness
level. Two axes are needed, and the source's lens questions (elegant,
disciplined, sincere, vulgar, memorable) ask for both.

### Obligatory categories [I]

Proposed grammatical commitments (the target-language selections that step 4
of `language-model.md` demands). Each names the packet field that feeds it.

1. **Finish marking** on predicates: an obligatory grade-class drawn from
   `stance.restraint`, coarsened to a few classes. Working classes:
   *raw*, *held*, *finished*. The unmarked form does not exist: leaving it
   off is not neutrality but a gap that the community reads as vulgar.
   Feeds from: `stance.restraint`.
2. **Leak channel**: how much felt affect shows *through* a high-restraint
   frame. Leak is not mapped to emotion words. It is mapped to **form
   choices**: clipped versus full inflection, length spent on a fricative,
   word choice within a field. This follows the standard's own example
   (anger leaking through clipped syntax is stance). Feeds from: felt affect
   grades against `restraint`.
3. **Standing agreement** on referents: field and standing-in-field of a
   named performer, with the **claim/demonstrated** mismatch statable. Feeds
   from: speaker and scene features (Section 6).
4. **Witness and form-evaluation**. Broadcast commentary is evaluative by
   situation. Evaluation is expressed as a **separate discourse layer**
   (a particle or clause layer), not as adjectives in the content: the
   lens questions [E] (elegant, disciplined, sincere, vulgar, memorable,
   promising, mastery, collapse) are the layer's open slots. It is
   kept apart from content because a held-back evaluation is not a claim
   about the world, and the interlingua keeps content and attitude
   separate. Feeds from: `discourse` features and the situation.
5. **Evidential**: commentary asserts what the camera sees and *infers* the
   performer's sincerity. The inference must be marked, because the source
   puts sincerity in the performer, who is not the speaker and not available to
   telemetry. Feeds from: the evidentiality feature the standard already
   names.

### Register is a function of three inputs [I]

Register is not a setting on the speaker. It is selected from the pair
(speaker's standing and community, scene relation to addressee and subject)
and the situation, with stance as the final modulator. This is what
`persona-drives-ontology` implies on `main` and what the
`language-ontology` cut will implement as deterministic selection
`select(profile, stance, speaker, scene)`. This spec supplies the **content
of that selection table** for this language; it does not write the table.

Cases the table must cover first:

| Case | Expected pull |
|---|---|
| Commentator, high restraint, strong approval of a restrained low-means act | finished marking; evaluative layer foregrounded; length withheld |
| Commentator, low restraint, strong approval | raw marking; the speaker is performing delight. Only permitted in tourist-adjacent registers |
| Master addressing rival | held or finished; address by field and standing |
| Tourist, any affect | claimant standing; over-lengthened; evaluation layer present but misapplied |
| Host to a human guest on a panel | warmth and appraisal mixed: affection plus contempt at high restraint is the *normal* pairing. The guest's rejection is absorbed as form |

The last row is the story's turn written as a stance mapping, and is worth
testing first: contempt and affection together, restrained, with the evaluation
layer calling the rejection *sincere*.

## 4. Meaning Before Surface

The language is never produced by substituting English words with alien
ones. The method is the Weksa path (`language-model.md`): packet, then
ontology projection that can split, collapse or reweight concepts, then
grammar commitments, morphology, phonology, surface.

### Concept splits and collapses [E→I]

The source names distinctions the language must make (`language-design.md`,
Open Questions). Each is decomposed below into components so a lowerer can
select a form from the packet's facts and not from a gloss.

| Distinction | Decomposition (hypothesis) | Needs from the packet |
|---|---|---|
| **victory** vs **mastery** vs **spectacle** vs **vulgar overmatch** | victory: outcome of a contest. mastery: outcome *plus* demonstrated chosen constraint. spectacle: attention drawn, independent of constraint. vulgar overmatch: means exceed the opposing means with no constraint chosen. | outcome; each party's means; whether the means were chosen |
| **crude** vs **failed** | crude: sincere, unrefined, with form attempted. failed: form collapsed into mere function. | actor's means and discipline as facts; the stance of the performer is not available, so the ontology reads from conduct |
| **resistance** admired and absorbable | resistance read as: threat, folk art, breakout talent (`battle-royale.md` commentary list). Choice depends on whether it centers a performer or a practice. | performer-centered vs distributed agency |
| **public spectacle** vs **living practice** [E] | spectacle: meaning lives in a central performer. practice: meaning lives in relationships. The empire reads distributed cooperation as noise or misattributes it to a performer. | number and relation of agents; presence of a focal agent |
| **unpainted**, split | shame / danger / contempt / rawness / refusal / purity (the source lists these senses). Different surface forms: the sense is selected by relation and stance, not by a gloss. | decoration state, scene relation, speaker stance |

### Scale decoration in grammar and metaphor [E→I]

Decoration is the language's central metaphor domain [E: used for rank,
grief, celebrity and more]. Proposed uses, to be tested not asserted:

- **Address**: terms of address name a decoration claim ("the one marked for
  ...") instead of a rank noun.
- **Insult**: lack, mismatch or fraud of decoration (a claim not backed by
  constraint). The strongest insult is the claimant who is also vulgar.
- **Praise**: finish, as in a surface completed.
- **Metaphor**: grief and obligation are *worn*. A human speaking of loss
  reads, in this language, as a person visibly undecorated by it.

The source does not say whether the language has these metaphors. They are
the draft's strongest invention and the easiest to overrule.

### Root families [I]

House method: faint family resemblance, never a nametag. No roots are
proposed here. The first root set should come from the field concepts
(combat, speech, hosting, trade, art) and the constraint/finish axis, and
should be minted against corpus items, not authored in a vacuum.

### Contamination direction [E]

What leaks into human English first (`language-design.md`) is an open
question. [I] Candidates are evaluation-layer particles (the empire's form
verdicts become meme tags) and the claim/finish vocabulary, because the
story's kids absorb *prestige codes* first. The subtitle renderer is a
separate target profile and not this spec's job.

## 5. Evidence and Invention, Summarized

**Evidence-backed (E):**
- reptilian, aesthetic-honor, decoration-as-status speakers; unpainted as
  profound shamelessness; unpainted-as-provocation for a rare few
- mastery requires chosen constraint; overmatch and tourists are vulgar; the
  lens questions
- the empire is a network; commerce carries, culture conquers; the registers
  listed
- broadcast commentary and flavor-first subtitles as the runtime use
- the packet has `stance` with felt affect and restraint; the ontology maps
  them (on `main`)

**Invention (I):**
- the two-axis status ladder and the claim/demonstrated-constraint mismatch
- the whole phonological inventory and prosody
- the five obligatory categories and the inverted restraint reading
- the decoration-grammar and metaphor uses
- the speech situations table, notably the address-to-humans row
- the contempt-plus-affection panel pairing

**Not settled in the source and left open:**
- the language's and speakers' names; the vocal tract; whether the empire is
  one language community or a family of dialects; whether non-reptilian
  imperial subjects speak it and how it marks them

## 6. What The Lowerer Must Provide

Interlingua rule [E]: the packet carries neutral meaning plus source-side
communicative pressure, and target-language concept splits belong
downstream. The earlier Rust to Dust sketch in `language-design.md` puts an
`aesthetic_reading` block (form, discipline, spectacle, honor status) **in
the packet**. This draft recommends moving it: those are the *imperial
ontology's reading* of neutral facts, so they belong in the ontology
projection, and the packet carries the facts the reading is derived from.
[I] This is a change to existing content and the operator should rule on it.

Fields, in order of need. "Seed" means the standard's inventory already has
a slot. "Gap" means the standard has no field yet; per the Promotion Rule,
the gap is first an `rtd.*` extension and is promoted only after use in two
real packets.

| Need | Why | Status |
|---|---|---|
| `stance` with felt affect and `restraint` | the whole of Section 3 | seed (on `main`) |
| **speaker** axes: field, standing in field, school or lineage, decoration state, community | status and register selection | seed slot `Speaker` (still planned, cut `speaker-scene`); the *values* are the gap |
| **scene**: relative status of speaker, subject and audience; audience composition (imperial, tourist, human via subtitles) | register and address | seed slot `Scene` (still planned, cut `speaker-scene`) |
| `medium` and `setting` ids for broadcast commentary | situation selection | seed (`context`) |
| `instrument` role on predications, **for each party** | overmatch is derived from both parties' means | seed role |
| **volition of the constraint**: chosen / compelled / unknown, as a predication feature | the line between mastery and mere lacking | gap (`rtd.` extension first) |
| **ordering of means** (power or means gap) | vulgar overmatch | not a packet field: ontology-side order over concept ids, so the packet carries concepts and the ontology compares them |
| referent **decoration state** (decorated / unpainted / bare-by-claim / not applicable) | status, insult, the human-undecorated reading | gap (`rtd.` extension first) |
| referent **community** or `member_of` relation | tourist versus warrior; human versus imperial | seed relation `member_of`; community ids come from the `language-ontology` cut |
| **focal agent** presence | spectacle versus living practice | derivable: a predication with plural or no agent role; not a new field |
| `evidentiality` | witnessed versus inferred sincerity | seed feature (to confirm its option set) |
| `polarity`, tense, aspect | ordinary | seed |
| `preserve` | which nodes the commentary must keep | seed (`constraints`) |

Fields the draft deliberately does **not** request: `aesthetic_reading`,
`honor_status`, `spectacle_value` or any value judgment. The performer's
sincerity is not telemetry, and the commentator infers it. These are
outputs of the imperial ontology and must not be put into the packet by the
client, or the client is deciding the language's concepts.

## 7. Dependencies, Risks and Next Cuts

**Dependencies.** Stance is on `main` (Weksa PR #3). Only the `Speaker` and `Scene`
slots are still planned, under cut `speaker-scene`. The `language-ontology` cut
(community and ontology-entry documents) is not landed. This spec states
the content those cuts will hold, and does not substitute for them.

**Risks.**
- The inventory and the grammar are heavily invented on thin evidence. Mitigation:
  tag everything (done) and test the five obligatory categories against the
  lowerer before building roots.
- The claim-versus-constraint ladder risks making the language a *status-code
  detector*. The source's hold is that honor is devotion, not rank. Test:
  can the grammar say a bare-by-claim performance without calling it low?
- Draft collided with `language-design.md`'s packet-side `aesthetic_reading`.
  Resolved by ruling `rtd-imperial-aesthetic-reading-to-ontology` (below).

**Next cuts, in order.**
1. (done) Operator rulings on the four forks, recorded above.
2. A first corpus slice: ten broadcast-commentary utterances, each decomposed
   to a packet (neutral facts plus stance), with the expected surface *category*
   (finish class, evaluation layer) and no words yet.
3. The selection table for Section 3's first five cases.
4. A root set from the field concepts, and a naming brief for the language.
5. An orthography and a pronunciation lowering.

**Rulings (2026-11-05, all in force).** The four forks are answered. Their
ids are in the mind's `weksa-body` campaign.
- `rtd-imperial-aesthetic-reading-to-ontology`: `aesthetic_reading` leaves the
  packet and lives in the imperial ontology. The packet carries neutral facts
  plus stance; form, discipline, spectacle and honor status are the ontology's
  reading of them. Section 6 stands.
- `rtd-imperial-two-axis-status`: status is two axes, field and standing, by
  claim versus demonstrated constraint.
- `rtd-imperial-inverted-restraint`: restraint is the display of virtue; low
  restraint with strong affect is vulgar.
- `rtd-imperial-human-stylized-tract`: the vocal tract is human-pronounceable
  stylization, with length as ornament.
