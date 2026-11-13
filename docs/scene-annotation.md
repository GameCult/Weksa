# Scene annotation

How the keeper scenes of `data/original/speaker-state.cc` are labelled. A scene
label has a committed rule that produced it; this file is that rule.

## Why

Two findings forced a written rule.

- `annotation-rank-from-register`: an annotator who reads the line's register
  (politeness forms, honorifics, endings) to decide the speaker's rank hands the
  lowering its own answer back, because register is what the lowering must
  produce from the scene.
- The addressee gate (`with_scene` in `crates/weksa/src/validate.rs`): a scene is
  read only when the packet names a discourse addressee. A line that addresses no
  one reads `NoAddressee` and no Scene entry selects, so labels on narration are
  inert and the only labels that bite are those on lines said to someone.

The earlier labels followed two briefs that were never committed and that
disagreed with each other (the relation to the reader, and a situation read with
the register ignored). The rubric below replaces both for every live group.

## Provenance

Cut `weksa-lowering:cut_spec:cut-scene-in-group.r1`, ruling
`weksa-lowering:ruling:scene-rubric-whole-bed`: all 98 live Train and Validation
groups of `data/original` receive a new scene revision from annotator A under
rubric r1, generator `claude-sonnet-5-5 (Weksa Hands, scene rubric r1,
docs/scene-annotation.md)`. Earlier revisions stay as provenance. The rubric uses
no corpus line as an example on purpose: a labelled example would make the blind
check against the old labels circular.

A later rubric revision gets a new section and a new generator string; r1 is
never edited.

## Rubric r1

The speaker of every line is the keeper, a quiet caretaker of a house and garden. First decide whether the line addresses a listener: it asks them something, asks them to do something, tells them about themselves, wishes something for them, or speaks to them as you. A line that addresses no one (narration, description, a remark about the world or about other people, a thought or wish with no one addressed) is told to the reader, a stranger: answer addressed false, relative_status level, intimacy 500, in_group false. People named inside a line are not its listener.

For a line that addresses a listener, answer addressed true and judge the keeper's relation to that listener from the situation the line states or implies, never from its register: politeness forms, honorifics, sentence endings, contractions, please and casual particles are what the lowering must produce from the scene, so they cannot decide it. relative_status: above when the situation gives the keeper standing over the listener (instructing, advising or permitting as elder, host, employer or teacher); below when it gives the listener standing over the keeper (the keeper serves, thanks, or asks a favour of a guest, employer, elder or official); level when it shows neither.

intimacy, 0 to 1000: 500 when the line shows nothing; up to 800 when it shows shared daily life (the listener's body, sleep or habits, the household's routine); above 800 only for a stated close bond; down to 200 for a stated first meeting or a service relation. in_group: true only when the line shows the listener belongs to the keeper's household, family or working circle, stated or plainly implied; a request, a question or a kind wish alone is not enough; otherwise false. Answer one JSON line per group, in the order given and nothing else: {"group": G, "addressed": true|false, "relative_status": S, "intimacy": N, "in_group": true|false}.

## Procedure

- Annotator A (committed) and annotator B (measurement only) are separate fresh
  model agents, one pair per language (49 live groups each). Each sees only the
  text of Rubric r1 above and that language's sentences as `group<TAB>"text"`,
  shuffled by a fixed seed. Neither sees an existing scene, the other annotator's
  answer, the ontology, which items are old or new, or any Test text.
- An output that does not parse or misses a group is re-drawn once and logged.
- A's labels are written with `weksa scene put` (one revision per group,
  `--to unnamed --inventory-rev 4`). `addressed` is not stored: no Scene feature
  carries it.
- Disagreements are reported, never adjudicated.
