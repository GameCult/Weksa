# Zyphos Name Briefs (DRAFT)

**Status: every name in this file is DRAFT. None is canon.** Per the
[README](./README.md), a name is canon only when a language project can explain
its ontology and form. No such project exists yet, so these are candidates held
against briefs, not entries in a lexicon.

Source: follow-up `weksa-body/zyphos-name-drafts`. Brief shape follows
[Language Design Pressure](./language-design.md#naming-brief-shape): the brief
comes before the name, and the name is the receipt. Setting context was read
from the Zyphos vault (`Species/Airawa`, `Species/Matriarch Trees`,
`Species/Sa'ueia`, `Civilizations/*`, `Ecology/Breeding Grounds`,
`Ecology/Nation Scale Interbeings`); the vault was not edited.

Both brief sets wait on Meta's answers to `vq-first-language` and
`vq-name-scope`. Where a brief's scope depends on them it says so.

## Deru's answers, 2026-11-06 (canon for Zyphos and Sa'ueia)

Source: Deru in #swarm, relayed in Weksa's memories of 2026-11-06 (12:03Z and
12:07Z). Each line is marked **settled** (Deru said it) or **invented** (this
file's reading). Where it overrides a brief or rule below, that text is marked
SUPERSEDED in place.

| Fact | Status |
| --- | --- |
| The main word comes first in a Sa'ueia compound (`ilawerum`: walk of knowledge). | **settled** |
| Sharing memories and knowledge is emphasised in Sa'ueia culture. No reputation attaches to the act of remembering, to the partner or to the memory. | **settled** |
| Each Airawa tribe has its own name, all linked to matriarch trees. | **settled** |
| Phonotactics, for **both** Airawa and Sa'ueia: two consonants never meet; two identical vowels never meet; no `q`, only `k` (so `qerath` is impossible). | **settled** |
| "`th` is not a cluster" (as relayed). | **settled**, but how far it extends is open: the checker reads it as `th` = one consonant. `sh`, `zh` and `kh` are treated as two consonants until Deru says otherwise. |
| The Airawa label for the Sa'ueia is one word, not unkind, with no discrimination. | **settled** |
| Individual names are given by matriarch trees, in both continents. Names are not inherited, only given through trees. | **settled** |
| High titles (shaman, leader) can serve as names. | **settled** |
| Airawa names can change after a deed meaningful to the whole tribe. Sa'ueia names change by council consensus. | **settled** |
| The remaining Airawa sound tendencies (hard plosives, textured sibilants, heavy stress) and the Sa'ueia open items (inventory, syllable shapes, stress, `weru` vs `wer-`, apostrophe role) stay as written. | unchanged, still **invented** or open |

What this retires, by id:

- **A1..A3 (Airawa) are about a tribe, not a people.** The pan-Airawa self-name
  question (`vq-name-scope`) is answered: names are per tribe, each linked to a
  matriarch tree. A1's text below still describes a pan-Airawa word and is
  **SUPERSEDED** in that respect; it is not yet rewritten as a per-tribe pattern.
  The tribes' actual names are Deru's.
- **S1 and S3 lose the reputation argument.** The briefs' reading that "standing
  is the partners' memory of generosity" and "being recognized as beneficial"
  conflicts with answer 2. The Sa'ueia value *sharing* memory and knowledge, so
  memory-as-currency is **SUPERSEDED**; `muna` ("memory") stays only as a
  neutral word for what is shared.
- **Individual names** (Zyphos names note, section "Individuals") are no longer
  blocked on "how names are earned": they are **given by matriarch trees**.
  Airawa: the tree gives the name and a deed can change it. Sa'ueia: the tree
  gives it and council consensus changes it. Titles may be used as names.
  Nothing is proposed for individuals beyond this pattern.
- **R-A4 is SUPERSEDED** (closed syllables): a closed syllable followed by a
  consonant-initial syllable puts two consonants together, which Deru forbids.
  Only a word-final consonant survives.
- **R-A1 and `qatar`-like forms:** `q` is gone; the first-contact slice's
  `qatar`, `daruq`, `karuq`, `qerath`, `thaleq` all need a `k` respelling.
  `Druzkai` breaks the adjacency rule (`dr`, `zk`) and `Serak` passes. Neither is
  canon.
- **R-S7 is SUPERSEDED** by Deru's two vowel and consonant rules: a doubled
  letter is now excluded wherever it appears, not only inside a morpheme.

Phonotactic check of every earlier candidate, with `sa-ueia/check.py`
(see `sa-ueia/names.yaml` for the current names, which all pass):

| Candidate | Result |
| --- | --- |
| `Qazhkar` | fails: `q`; `zh`, `hk` as adjacent consonants |
| `Dozkai` | fails: `zk` |
| `Tegzhir` | fails: `gz`, `zh` |
| `Kharzdun` | fails: `kh`, `rz`, `zd` |
| `Qetoz` | fails: `q` |
| `Gisqat` | fails: `q`, `sq` |
| `Zhedkor` | fails: `zh`, `dk` |
| `Tkarzet` | fails: `tk`, `rz` |
| `Ellumaiwa` | fails: `ll` |
| `Sauwele` | passes (but still too close to `Sa'ueia`, see S1) |
| `Oluneihe` | fails: `h` (already withdrawn) |
| `Druzkai` | fails: `dr`, `zk` |

The Zyphos names note (PR 2) proposed six Airawa names from closed-syllable
roots; all six fail the same rules and are replaced there by names built from
open roots. Those roots are Weksa placeholders and `kerath` is the slice's
`qerath` with `q` replaced and `th` kept: both as invented as before.

## What the sound rules actually give us

`language-design.md` is thin, and the briefs should not pretend otherwise.

- **Airawa** has a provisional list (R-A below). It is a tendency list, not a
  phonology: no vowel inventory, no photactic limits, no stress rule beyond
  "heavy on socially important forms".
- **Sa'ueia has a working sound section**
  ([Sa'ueia Sound](./language-design.md#saueia-sound-proposal-for-deru-to-cut),
  enforced by `sa-ueia/check.py` against `sa-ueia/table.yaml`). It is a
  proposal Deru approved in outline; only the species name is canon. Rule
  labels R-S4..R-S8 below cite it. The consonant inventory, syllable shapes and
  stress are still open, so sound arguments beyond the letter rules stay weak.
- The **imperial Airawa register** has no sound rules either. It is only
  described as "an artificial quarter-continent clausiform exception imposed
  through biological-memetic control", not "named like a normal state with wet
  nouns glued on".

Rule labels used below (quoted or paraphrased from `language-design.md`):

| Label | Rule |
| --- | --- |
| R-A1 | Airawa: dorsal and coronal plosives prominent (`k g t d`; `q` retired, Deru 2026-11-06). |
| R-A2 | Airawa: sibilants and postalveolars carry social texture (`s z sh zh`). |
| R-A3 | Airawa: `r` may be trilled in some registers. |
| R-A4 | SUPERSEDED (Deru, 2026-11-06): no two consonants meet, so only a word-final consonant closes a syllable. |
| R-A5 | Native Airawa names must not read like smooth Sa'ueia route names unless explicitly loan, exonym or cross-continental register. |
| R-A6 | Room consensus: hard edges, clipped/closed syllables, heavy stress on socially important forms, so claims land with weight. |
| R-S1 | Sa'ueia is softer than Airawa. |
| R-S2 | Sa'ueia forms flow longer and wetter than Airawa. |
| R-S3 | Sa'ueia route names read smooth. |
| R-S4 | Sa'ueia letters: vowels `a e i o u`; forbidden `h q y` (`k` where a `q` might have been). `q` is forbidden in Airawa too (Deru, 2026-11-06). |
| R-S5 | Sa'ueia: short words, soft and open; at most 10 letters joined (warning, Weksa placeholder). |
| R-S6 | Sa'ueia: suffixing morphology; identical letters merge at a join (`weru`+`um` -> `werum`). |
| R-S7 | SUPERSEDED (Deru, 2026-11-06): no two consonants and no two identical vowels meet, anywhere, in Sa'ueia and Airawa. |
| R-S8 | Sa'ueia roots from Deru: `ila` walk, `weru` know, `olune` breeding ground (unanalyzed), `-um` verb suffix. Composed: `werum`, `ilawerum` (walk of knowledge). |
| R-N | Naming rule: meaning decomposition first; define the speaker's scale, name type (endonym, exonym, ritual avoidance, technical, imperial administrative, route-name, child-name, trade gloss), what the speaker's ontology makes obligatory, and whether the name is stable, seasonal, situational, inherited or evasive. |

Existing scratch evidence to keep in view: `Druzkai` (Face for a non-imperial
archivist) and the first-contact slice lexicon (`atar`, `qatar`, `daruq`,
`karuq`, `qerath`, `thaleq`...). Neither is canon either.

Each target's brief ends with **reject if** conditions. A name that fails one
goes back to the brief, not to a patch.

---

## Airawa, non-imperial

Common to all Airawa briefs below, unless one overrides it: the speakers are
pre-imperial or never-imperial Airawa, in the `Pre-Imperial Airawa Tribes` /
native nation-scale sense, not the Empire. Registers (Airawa imperial, native
tribal, mother-tree-facing) must not collapse into one naming soup
(`README.md`, Authority).

### A1. Airawa self-name (candidates: `Qazhkar`, `Dozkai`, `Tegzhir`) — DRAFT

```yaml
target:
  kind: endonym (what a non-imperial Airawa people call themselves)
  canon_source: Species/Airawa; Civilizations/Pre-Imperial Airawa Tribes
  provisional_english_handle: "the Airawa, as they name themselves"
namer:
  language_project: none yet (non-imperial Airawa, native register)
  register: formal naming register (slice: `qatar`, formal name), not casual
  relationship_to_target: the speaker is the target
```

- **Speaker:** a non-imperial Airawa speaking as a people, or as a tribe
  inside the people (open: see scope question).
- **Addressee:** primarily other Airawa and witnesses at formal naming; second,
  an outsider receiving an introduction (the first-contact scene).
- **Register:** formal, standing-bearing; the kind of word that follows `atar`
  ("I stand as") and has to land with weight (R-A6).
- **Meaning decomposed before surface:**
  a self-name here is probably not "people" in the English sense. The setting
  makes the unit of selfhood a relationship: a lineage held by a mother tree,
  bound by gestation contracts, contract dress and nutrient obligation. The
  candidate meaning bundle is: *those held in memory by mother trees* /
  *those who owe and are owed* / *kin through the archive*. It must also work
  as a contrast with the empire, which edits memory: the pre-imperial self-name
  should presuppose negotiated, leaky membership, not a uniform nation.
  Scope question (needs `vq-name-scope`): one pan-Airawa word, a per-tribe
  word, or a word for the *kind* of polity (nation-scale interbeing, which
  includes the trees)?
- **Sound rules that apply:** R-A1, R-A2, R-A4, R-A6; R-A5 as a negative check.
- **Per-candidate sound read (judgment, not rule):**
  - `Qazhkar`: `q` + `zh` + `k`, two closed syllables (Qazh-kar), fits R-A1,
    R-A2, R-A4, R-A6 cleanly. Shares `-kar` with `sakar` and `kazhar` from the
    slice. That is a plus if `-kar` is meant to be a role/person suffix and a
    minus if it was only a sound resemblance.
  - `Dozkai`: closed first syllable, open second ending in a diphthong `ai`.
    Fits R-A1/R-A2 but the open `-ai` tail is the softer shape R-A4 contrasts
    against. Also two edits from the scratch name `Druzkai` (insert `r`, `o` to
    `u`); two names with that near neighbourhood will blur unless one is explicitly derived from
    the other.
  - `Tegzhir`: `t-g-zh` cluster, closed final syllable, clean fit. The only
    candidate with a front vowel in the stressed syllable, which may or may not
    be a feature.
- **Reject if:**
  - the meaning bundle above cannot be stated in one clause that names the
    mother-tree relationship (a bare "people" or "the strong" gloss);
  - the form also reads as a plausible Sa'ueia word (R-A5);
  - it is shared, as a root, with the imperial exonym in A4 without a recorded
    reason (loan or deliberate echo);
  - the project owner decides the self-name is per-tribe and the word is only
    pan-continental.
- **Preferred shape of choice (not a ruling):** pick by the meaning bundle, not
  by taste; all three pass the sound list, so sound is not a discriminator here.

### A2. Mother tree (candidates: `Kharzdun`, `Qetoz`) — DRAFT

```yaml
target:
  kind: name for a being / kind of being, not a place
  canon_source: Species/Matriarch Trees; Ecology/Mother Tree Networks
  provisional_english_handle: "mother tree / matriarch tree"
```

- **Speaker:** non-imperial Airawa. Possibly two speakers needing two words:
  a tribe naming *its* tree, and a general word for the kind. Open.
- **Addressee:** other Airawa (referring to the tree); *not* the tree itself.
  The slice requires distinguishing "direct speech to a mother-tree" from
  "authorized delivery to it", so referring and addressing probably want
  different forms.
- **Register:** archival / ritual-respectful. Not a diminutive and not a
  kinship term.
- **Meaning decomposed before surface:** the English handle "mother" imports
  nurture. The vault's tree is a gestator *and* a memory-holder *and* a party
  with coercive leverage (memory can be held hostage; defying it costs
  ancestral continuity). The Airawa name should carry at least: *keeper of
  memory* and *gestation contract*, and should not smuggle in "gentle". Whether
  the name also marks the tree as a sovereign party (a polity, since it is a
  node in a nation-scale interbeing) is a fork for the owner.
- **Sound rules that apply:** R-A1, R-A2, R-A4, R-A6.
- **Per-candidate sound read:**
  - `Kharzdun`: the `kh` is outside the R-A1/R-A2 list (`k g q t d s z sh zh`);
    it needs either a rule admitting it or a respelling. Three syllables with
    `rz` cluster and final `-dun`: heavy, closed. Strong fit to R-A6 otherwise.
  - `Qetoz`: `q` + `t` + `z`, two syllables, fits R-A1/R-A2; the open middle
    vowel pattern (`e`, `o`) is lighter than `Kharzdun`. Shorter is arguably
    right for a word said often; longer for a word said with weight.
- **Reject if:** the name decomposes only to "mother"/"nurse"/"grandmother"
  (kinship nostalgia without the contract and coercion); it is the same word
  the empire uses for the tree as *infrastructure* (the Biological Memetic
  Engine reuses the tree network, so the empire's word is a different brief);
  the project owner rules the kind-word and the individual-tree-word must be
  separate and only one is given.

### A3. Disconnected network (candidates: `Gisqat`, `Zhedkor`) — DRAFT

```yaml
target:
  kind: name for a surviving mother-tree ecology that severed itself
  canon_source: Civilizations/Disconnected Tree Networks
  provisional_english_handle: "disconnected (tree) network"
```

- **Speaker:** a native non-imperial Airawa, ideally someone inside or allied
  with such a network. **Not the empire**: the empire denies they exist
  (`Airawa Empire`, "Nations It Refuses to Recognize"; they are called
  "disease, criminal ecology, or imperial tissue awaiting correction"), so the
  imperial register has no sanctioned word, and any imperial term is a
  separate brief (A4).
- **Addressee:** other native Airawa; possibly a visiting envoy (the vault has
  envoys crossing into Sa'ueia territory). Rumor-grade circulation: the networks
  "keep rumors moving through routes the engine cannot fully process".
- **Register:** careful. The vault has the status as survival, refusal and
  wound at once; the name may need an indirect/evasive form
  (`README.md`: "safe-to-name, indirect-name-required, silence-required").
- **Meaning decomposed before surface:** *severed* is the surface fact; the
  contentful meaning is *alive because cut*: the connection was surrendered to
  preserve the self. A gloss of "isolated" or "cut-off" loses that. Axes to
  decide: is the name about the cut (act), the survivor (state), or the
  continued resistance (role)? Do networks of "trees with more control than
  people" and "anti-imperial Airawa communities" share one word, or does the
  word name the property (disconnection) rather than the polity?
- **Sound rules that apply:** R-A1, R-A2, R-A4, R-A6.
- **Per-candidate sound read:**
  - `Gisqat`: `g-s-q-t`, two closed syllables; a hard stop sequence that reads
    as something cut off, which is apt but also a sound-symbolic gloss the
    project should not over-trust.
  - `Zhedkor`: `zh-d-k`, trilled `r` available (R-A3); heavy and closed.
- **Reject if:** the form is also the imperial term (the empire's denial means
  the two registers should not share one word without cause); it glosses to
  "dead", "lost" or "isolated"; the owner decides the target is a *property*
  word (adjective for any severed tree) and the candidate is a *noun for the
  polity*, or vice versa.

### A4. Imperial exonym for the natives (`Tkarzet`) — DRAFT

```yaml
target:
  kind: exonym, imperial administrative
  canon_source: Civilizations/Airawa Empire; Conflicts/Xenophobic Memetic Campaign
  provisional_english_handle: "the natives / unincorporated Airawa"
namer:
  language_project: none (imperial Airawa register)
  register: imperial administrative / doctrinal
  relationship_to_target: classifier, denier
```

- **Speaker:** the Empire, an artificial nation-scale interbeing speaking
  through ministries, doctrine and memetic organisms.
- **Addressee:** imperial citizens and administrators; also doctrine-carrying
  memes. Not the natives: it is not a word the natives would be called by to
  their faces in a respectful exchange.
- **Register:** official. The language-design rule says this register is
  "an artificial quarter-continent clausiform exception", not a normal state's
  wet-noun coinage.
- **Meaning decomposed before surface:** the empire's stance is
  *non-recognition*: native nations are "disease, criminal ecology, or
  imperial tissue awaiting correction"; unification is "biologically
  inevitable". So the exonym should classify natives as *not-yet-incorporated
  tissue* or *contamination*, not as a people who happen to be foreign. It
  should be usable in doctrine and in accusation (dissenters are "infected").
  Open: does the word split native Airawa from the disconnected networks (A3)
  or is it a single catch-all? The empire denies the networks exist, so the
  catch-all risks naming what the doctrine says does not exist.
- **Sound rules that apply:** R-N (exonym). The provisional R-A list governs
  *native* Airawa, not the imperial register. There is no imperial phonology
  rule yet, so any sound argument for `Tkarzet` is by analogy to R-A1/R-A2.
  `Tkarzet` opens with a `tk` cluster that appears nowhere else in the
  candidate set or the slice lexicon; it either marks the imperial register as
  phonotactically different (good if intended) or is a stray.
- **Reject if:** it is a bare monosyllabic-looking root with no clausal or
  administrative structure (the imperial register is clausiform); it is simply
  a native word borrowed with harsher spelling (R-A5's reverse: the imperial
  word should not accidentally be a native one); it gives the natives a
  sovereign-people gloss, contradicting the doctrine; the owner decides the
  imperial register needs a distinct sound system and the candidate cannot be
  defended against it.

---

## Sa'ueia family-unit and ecological contexts

Common context: the Sa'ueia civilization is mobile, family-unit based
(squad-sized groups doing kinship, craft, diplomacy and oaths together), with
sacred communal breeding grounds and reciprocal ecological partners. A name
earns its place by revealing the ecological deal, reproductive pressure,
memory technology or route consequence (`language-design.md`, design target).
The sound rules available are R-S1..R-S8 and R-A5; the inventory, syllable
shapes and stress are still open.

### S1. Sa'ueia self-name (candidates: `Ellumaiwa`, `Sauwele`) — DRAFT

```yaml
target:
  kind: endonym; family-unit scale, not species scale (open)
  canon_source: Species/Sa'ueia; Civilizations/Sa'ueia Family Units
  provisional_english_handle: "the Sa'ueia as a family-unit people"
```

- **Speaker:** a Sa'ueia family unit speaking as itself (family-self scale),
  possibly on behalf of the wider people.
- **Addressee:** other family units meeting on a route or at a breeding
  ground; and ecological partners that keep memory of it (reputation is
  held by partners, not only by humans).
- **Register:** family-unit speech, relational and portable; a name that
  survives travel and can be re-presented to a partner that remembers you.
- **Meaning decomposed before surface:** the vault makes the family unit the
  carrier of standing, and standing is the partners' memory of generosity. So
  a self-name probably means something like *those who return and are
  recognized as beneficial* / *the traveling reciprocal*, not *the people of a
  place*. Scope fork: the continent-scale word "Sa'ueia" already exists in the
  vault as the species/continent ethnonym; the candidates may be (a) family-unit
  *kind* names, (b) nicknames for the people derived from it, or (c) a
  competing ethnonym. This needs `vq-name-scope` before judging.
- **Sound rules that apply:** R-S1, R-S2, R-S4, R-S5, R-S7; R-A5 as a negative check.
- **Per-candidate read (judgment, then the letter rules):**
  - `Ellumaiwa`: long, open, liquid-heavy. Fails R-S7: `ll` is a doubled
    letter inside a morpheme, with no join to merge it, unless the form
    is parsed as `el`+`lumaiwa` (then the merge rule would give `elumaiwa`).
    Also 9 letters, near the R-S5 bound. Needs a respelling or a parse, not a
    patch.
  - `Sauwele`: short, open vowels and `w`, `l`; passes R-S4/R-S5/R-S7, but it
    begins `Sa-u-` exactly like `Sa'ueia`. Reads as a respelling or derivation
    of the existing name rather than an independent self-name.
- **Reject if:** the owner has not decided the target scale; `Sauwele` is
  accepted without a recorded derivation from `Sa'ueia` (otherwise it is an
  accidental duplicate); either name decomposes to "wolf-like", "pack" or
  "nomad" (the vault warns the wolf comparison explains only part of them, and
  "nomad" is a camera-angle word); it cannot take a family-specific modifier
  when a particular unit needs naming.

### S2. Breeding ground (`Olune`, Deru's root; `Oluneihe` withdrawn) — DRAFT

```yaml
target:
  kind: place-name / commons name, ritual register
  canon_source: Ecology/Breeding Grounds
  provisional_english_handle: "breeding ground"
```

- **Speaker:** a Sa'ueia family unit returning to a ground; and the ground's
  year-round occupants (infants, infirm, caretakers, rotating specialists).
- **Addressee:** other family units, partners that remember them, and the
  occupants when arrival is announced.
- **Register:** ritual-communal; the vault gives these as places of
  transition, mourning, dispute settlement, medical care and obligation
  renewal, so the name has some solemnity.
- **Meaning decomposed before surface:** *commons* is the key axis: it is not
  property, not a seat, not a nursery run by an empire. The name should mark a
  place that *remains* (occupied all year) while the people circulate, and that
  gathers reproduction, care, memory and law. It must distinguish itself from
  camp, route and partner site (the Simulation Brief explicitly asks what makes
  these visibly different). If the name is a *kind* name, it has to be
  generic enough to name every ground; if it names one ground, it needs a
  proper-name layer.
- **Sound rules that apply:** R-S1, R-S2, R-S3, R-S4, R-S8.
- **Read:** the target already has a name from Deru: `olune`, breeding ground
  (R-S8), unanalyzed. The earlier candidate `Oluneihe` is withdrawn: it
  contains `h`, which R-S4 forbids, and its stem `Olune` is Deru's own root, so
  the candidate only added a suffix to a word that already answers the brief.
  What remains open for this brief is whether `olune` is the kind-word, a
  single ground's name, or needs a proper-name layer; and its morpheme
  boundaries, which Deru has not given.
- **Reject if:** any proposed form contains `h`, `q` or `y`; it decomposes to
  "home", "village", "city" or "capital" (the breeding ground failing as a
  capital is a stated system failure); it equals or contains the route word
  (S3) or the family self-name (S1), which would merge distinct targets; it is
  used for a camp; it replaces `olune` without Deru's say.

### S3. Route as obligation (no candidate) — DRAFT

```yaml
target:
  kind: route-name; relation, not geography
  canon_source: Themes/Ecological Bargaining; Ecology/Reciprocal Ecologies
  provisional_english_handle: "route (as owed relationship)"
```

- **Speaker:** a Sa'ueia family unit that holds or renews a route.
- **Addressee:** other units; the ecological partners (feeding sites, groves,
  herds, pollinator corridors) that keep memory of how the unit travelled it.
- **Register:** family-unit speech with some formality: this is a promise-bearing
  word. Possibly a seasonal/situational name (a route can open or close
  depending on remembered injury).
- **Meaning decomposed before surface:** the key point is that *the route is a
  relationship that makes passage available*, not a path on the ground. A unit
  may be physically able to travel yet unwelcome (`Family Units`). The word
  should mean something like *the standing that lets us pass*; ideally it
  distinguishes *the way* from *the debt*. If both senses are one word, the
  language is making a commitment about their identity that the vault does
  not make.
- **Sound rules that apply:** R-S2, R-S3 (route names read smooth), R-S4..R-S8. Also R-A5:
  Sa'ueia route names are what Airawa native names must not accidentally
  resemble.
- **Read:** `Ilawerum` is not a candidate. It is Deru's own composition,
  `ila` (walk) + `werum` (knowledge, verb form `weru`+`um`), glossed "walk of
  knowledge" (R-S8). It names a different thing, and the checker reproduces it
  as a fixed composition. Any route-as-obligation word should be built from
  Deru's roots or new roots she accepts, and must not be confused with, or
  collide with, `ilawerum`.
- **Reject if:** it is `ilawerum` or only a respelling of it; it is a plain toponym (no debt sense); it is the same word as
  the breeding ground (S2) or shares its stem without a morphological reason;
  the owner decides route and obligation are two words and this is only one.

---

## Questions this raises (owners, not decisions)

- **Scope:** one word per ethnonym or per tribe/unit? (`vq-name-scope`.)
- **First language:** Airawa vs imperial Airawa vs Sa'ueia vs a narrow
  naming-brief interlingua (`vq-first-language`; `state/map.yaml`). The Airawa
  briefs are better supported by existing sound evidence; the Sa'ueia briefs
  have only the working sound section, which leaves the inventory open.
- **Sa'ueia open items** (inventory, syllable shapes, stress, apostrophe role,
  `weru` versus `wer-`) are Deru's; S1-S3 are judged on the letter rules only
  until she cuts them.
- **Imperial register sound rules** for A4.
- **`kh` in `Kharzdun`**: add to the Airawa inventory or respell.
