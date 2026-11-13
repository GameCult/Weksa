# Name-span annotation

How the name spans of an imported public-domain work are placed. A span has a
committed rule that produced it; this file is that rule.

## Why

- `pd-annotation-model-then-attest`: a labelled model places the spans of a
  public-domain work; the operator may attest them afterwards.
- `leak-owned-by-flow-and-scorer-3`: spans are fixed with the text at intake and
  written by no loop writer. The offsets are derived from markup the annotator
  writes, never typed by a model (inline marking, as in GPT-NER, Wang et al.
  2023); `CorpusStore::add` judges range, grapheme boundary, form equality and
  overlap.

The work file (`weksa corpus import <work>`) is a transcription of a work in
hand: provenance, cast forms, lines, scene groups and the speaker each speech
prefix names. It holds no distributor header, licence or trademark text, and the
binary fetches nothing.

## Rubric r1

You are given a work's cast: each member's entity id and the forms the work spells that member with. For each numbered line, copy the line exactly, character for character, and wrap each place where the line names a cast member by one of that member's listed forms as [[<entity id>|<form>]]. Mark a form only where it refers to that cast member; a listed form used as an ordinary word, or naming someone else, stays unmarked. Do not mark pronouns, titles or descriptions unless the exact text is a listed form. When a form is listed for two members, mark the one the line refers to; when you cannot tell, leave it unmarked. Answer one JSON line per input line, in the order given, and nothing else: {"line": N, "marked": "..."}.

## Procedure

- One fresh model agent per work. Its brief is rubric r1, the cast as
  `<entity id>: <form>, <form>` lines and the work's lines as `<n><TAB><text>`;
  nothing else (no store, no loop, no other answer).
- Import with `--model-family` and `--generator '<annotator model id> (name-span
  rubric r1, docs/name-span-annotation.md)'`. An operator-written or
  operator-reviewed answer imports with `--operator`.
- Import is all or nothing: every entity and item passes the real store doors on
  scratch copies before the first write, so a refusal (a parse failure, a span
  the store judges, a work that repeats a line under case or width folding)
  leaves the stores untouched. Re-draw the whole answer once, log it, and import
  again. A work file repeating a line's text is refused naming both lines; leave
  that line out of the answer file (the rest imports with its numbering intact),
  since a repeated line cannot be two corpus items.
- A line the answer does not cover is not imported by that run; a later run over
  other lines of the work does not collide (keys are `item:<slug>:<n>`).
- A line whose own text holds `[[` or `]]` cannot be marked and is refused; it
  can still enter by hand through `weksa corpus add --name`.

## Label scope

`names_by` labels the spans only, not the line or its speaker. One label per
item: a later attestation would be a superseding item with the same text and
group, but no command writes one yet; question `pd-name-span-attestation` owns
that door.
`None` means unrecorded (the cast seeds, `weksa corpus add`, the Dao De Jing
bed), not attested.
