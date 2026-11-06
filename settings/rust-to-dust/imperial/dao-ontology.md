# Rust to Dust Imperial Ontology: Dao And Path-Fit

Status: **DRAFT r0.** Companion to [language-design.md](language-design.md).
Nothing here is canon until the operator rules on it.

## How To Read This

Claims carry the same tags as `language-design.md`:

- **[E]** evidence: the source says it. Sources are Meta's words in #weksa on
  2026-11-06 (the Dao placeholder, the path-fit norm-breaking examples and
  the Dao De Jing analogy) or a cited file under `settings/rust-to-dust/`.
- **[I]** invention: this draft proposes it; the operator can overrule it.
- **[E→I]** extension: a source seed this draft develops.

## 1. The Value System Is Path-Fit, Not Shame And Honour

[E] The empire's value system centres on one concept that human words only
loosely map. Its working label is **Dao**, a **placeholder** that Meta accepted
until a native name and sound exist. It means living well: roughly
self-actualization. Everyone is given gifts so that they may be used; getting
better at what one does is close to divine; presenting oneself is something
everyone must do.

[E→I] What follows, and what this ontology therefore does *not* use:

- **No audience sanction.** The measure is not what others may think (shame,
  face, honour as reputation). It is whether the act is true to the actor's own
  Dao. A speaker with no audience is still measurable against it.
- **No inner-state guilt.** Failing the Dao is *waste of a gift*, not a feeling.
  Stance grades named after human shame, pride or honour are placeholders for
  readings this ontology has not defined (see section 5).
- **Duty, not display.** Painting is not vanity; it is the duty of
  presenting oneself, a part of using the gift [E].

This replaces the shame/honour framing that earlier drafts leaned on
(`language-design.md` section 1, "shameless"; section 4, the *unpainted* split;
and `aliens.md`, "profound shamelessness"). The source file `aliens.md` is the
setting owner's and is not edited here; the tension is listed in section 6.

## 2. The Path-Fit Test

[E] Going unpainted is heresy by default (it is not being one's best self),
**unless** it expresses the speaker's own path. Examples from Meta:

- A warrior who goes into battle unpainted to show martial purity: valid.
- Breaking a norm (unpainted, obscenity) is valid when it emphasises the point
  or expresses the speaker's own path.
- A celebrity doing it as a statement: reads as **gauche**, not brave.

[I] So the ontology needs a test, not a flat rule. A norm-breaking act is read
by **fit between the act and the actor's path**:

| Input (all typed facts) | Slot it comes from |
|---|---|
| the norm broken (decoration absent, obscenity, ...) | packet predication, or referent decoration state (a gap: `rtd.` extension first, see `language-design.md` section 6) |
| the actor's **path**: field and what the actor's gift and practice are | `Speaker` slot (planned, cut `speaker-scene`) |
| whether the act **serves the actor's own point** (the aim it emphasises) | not a packet fact; read by the ontology from the act and the path, never put in by the client |
| scene: who watches, and whether the act is staged *for* them | `Scene` slot (planned) |

The output is a **reading**, chosen by the ontology, never sent in a packet:

| Reading | When [I] |
|---|---|
| **devout** | the norm is kept, the gift is shown |
| **heretical** | the norm is broken and nothing in the actor's path calls for it |
| **path-true** | the norm is broken and the act expresses the actor's own path (the warrior) |
| **gauche** | the norm is broken as a statement the path does not carry; the act borrows the *look* of path-true without the path (the celebrity) |

[I] Two properties keep this from becoming a status detector:

1. *Gauche* is a fit failure, not a rank. A high-standing actor can be gauche in
   one act and path-true in another.
2. The test is judged against the **actor's** path, not the speaker's. The
   commentator infers it from conduct and marks the inference as inferred
   (`evidentiality`, `language-design.md` section 3, category 5), because the
   performer's sincerity is not telemetry.

[I] Open: whether the same test governs obscenity and other norm-breaking, or
only the unpainted case. Meta named both as examples [E]; the shared test is
this draft's generalisation.

## 3. Native Terms: Context-Tagged Readings, Chosen Gloss Recorded Apart

[E, Meta's Dao De Jing analogy] A native term is not one English equivalent.
Model it as a term with **several readings, each tagged with the context that
selects it**, and record the **gloss chosen for a given lowering separately**,
so the choice can be audited and changed without rewriting the term.

Two records, held apart [I]:

**Term** (the imperial ontology's own, language-side):

- `term`: a native id. Until a name exists it is an opaque placeholder; `Dao`
  is the placeholder label for the first term, and is not a spelling.
- `readings`: each is `{ context, sense }`, where `context` is a typed
  condition over feature values (speaker path, scene, stance, packet facts) and
  `sense` is a short description of the meaning in that context. No reading
  names an English word as its meaning.
- Selection among readings is deterministic from `context`; if none matches the
  term is left unlowered, not defaulted.

**Gloss choice** (per lowering, target-side):

- `{ term, reading, target_language, chosen_gloss, why }`.
- It lives with the lowering that made it, not in the term. Two lowerings of one
  term into the same language may choose different glosses, and the term does
  not change.

First term, all readings [I] unless tagged:

| Term | Reading context | Sense |
|---|---|---|
| `Dao` (placeholder) | general | living well: the full use of one's given gifts; self-actualization [E] |
| `Dao` | the actor's practice improving | getting better at one's work as near-divine [E] |
| `Dao` | presenting oneself | the duty of presenting oneself, painted or path-true bare [E→I] |
| `Dao` | norm broken, path-fit | the actor's own path, as grounds for an exception [E→I] |

| Gloss choices (illustration only) | |
|---|---|
| `Dao`, general, en-US | "the way", chosen for a reader who knows the analogy; recorded as a choice, not as the meaning [I] |

[E→I] Why: the analogy Meta drew is the Dao De Jing's many human
translations, each choosing a gloss for one native term. Recording the choice
beside, not inside, the term is what lets that translation test bed (queued,
not started) compare lowerings of one term.

## 4. What This Gives The Lowerer

[I] Nothing new in the packet. The reading is selected from `stance`, `Speaker`
and `Scene` features (see `LoweringInput::with_scene` for the scene side) by the
planned `select(profile, stance, speaker, scene)`; the packet carries facts, not
*heretical* or *gauche*. This keeps the lowerer from English-to-alien
substitution: no English word is a key.

## 5. Placeholders

[I] These stance-adjacent names are **placeholders** for readings this ontology
has not defined, and must not be read as human shame/honour:

- `shame` (seed stance feature): in the inventory today as a felt-affect grade.
  Whether the empire has a felt affect of this shape, or only the *waste of a
  gift* reading above, is open.
- `contempt` and `affection` in the example stance (`panel` pairing, section 3
  of `language-design.md`): human labels for a pairing whose imperial
  reading is unsettled.
- "honor" and "vulgar" as used in the existing spec: placeholders for
  path-fit success and failure.

## 6. Open

- The native name and sound of Dao.
- Whether the unpainted exception reads as sincere or suspect to the commentator
  is the question Meta was asked; the answer so far is *path-true* is sincere
  and *gauche* is not [E→I], but how a commentator tells them apart from
  conduct alone is open.
- `aliens.md` says unpainted is "profound shamelessness" and a rare few go
  unpainted "as a controversial artistic claim": the first is the baseline
  reading this draft re-frames as heresy against Dao; the second is close to
  *path-true*. The setting owner should rule on whether to reword the source.
- Whether *devout/heretical/path-true/gauche* are the right four, or a
  continuous fit grade.
