# Calibrating the accept rule

`weksa loop calibrate` measures how the live accept rule behaves on a run's own
Validation corpus: how often it accepts, rejects as no gain, or regresses a
candidate that should be none of those. It does not reimplement the rule. It
reads the answered draws through `RunStores::read_draws` (the scoring
`weksa loop relower` uses), decides every trial with `decide_panel` over
`paired_floor` (`crates/weksa/src/run.rs`), and prints, per class, the items
read and skipped per language, the trial totals with their Accepted, NoGain and
Regressed counts, and the mean panel noise. That is all it produces: the
Accepted/NoGain/Regressed counts and the noise mean in the map's "Recalibration
on the grown corpus" block are reproduced by it, and the prompted trials are
also printed split by the recorded lowering `R`'s role (`prompted-r-candidate`,
`prompted-r-base`, `prompted-r-free`, see below); the other figures there (the
35% with R removed, the noise range, the 0.0185 gain and its 0.41 ratio, the
bootstrap intervals, the per-item SDs, R's +0.024, the mild-arm duplicate
counts and mean composite offsets, the Train-vocabulary overlap, the old/new
class composites and the ten parse-refused item ids) were computed by scratch scripts that are not in the repo. It is a
diagnostic: it records nothing, writes no prompts, and never touches Test or
Train.

## What it counts

The draws are lowerings of one round's Validation packets: the round's own
(recorded) lowering `R`, plus draws you label. `P` is `R` and the `--prompted`
draws; the floor size is `FLOOR_ARMS_MIN` (3), the live rule's.

- Prompted trials: every ordered pair (base, candidate) in `P`, against every
  3-subset of the rest of `P` as the floor. A repeat of the same lowering
  should read NoGain; Accepted is a false accept, Regressed a false reject.
  (`R` comes from the round, not from `relower`, so it may not be
  exchangeable with a repeat.) The prompted line is therefore printed three
  more times, by `R`'s role in the trial: `prompted-r-candidate` (`R` is the
  candidate), `prompted-r-base` (`R` is the base) and `prompted-r-free` (`R` is
  neither; it may sit in the floor). The three sum to the prompted line. Read the Accepted
  and Regressed columns of each line: a false accept sits mostly under the
  first and a false reject under the second, but `R`-as-candidate trials can
  Regress too (class `old` on `growth-scene`: 5 of its 15). The third
  is the exchangeable rate: on `growth-scene` it was 0 Accepted and 0 Regressed
  of 80 while all 11 false accepts and 9 false rejects involved `R`. Which of
  the two rates the live rule should be held to is undecided: the live loop
  meets `R` as the head's establishing lowering.
- Degraded trials, per `--degraded` arm: every base in `P` against every
  3-subset of the rest of `P`, the degraded draw as the candidate. These should
  read Regressed.
- Candidate trials, pooled over the `--candidate` arms: the same, tallied
  Accepted, NoGain, Regressed. This is the rule's power against that arm.
- Noise: the mean `panel_noise` over every (base, floor) choice.

With six prompted draws including `R` that is 120 prompted trials, 60 per
degraded arm and 240 candidate trials. Trials share draws and are not
independent. Each count is printed for `all` Validation items and for each
`--class NAME=PREFIX[,PREFIX...]` (items whose key starts with any prefix), from
the same per-item scores. A class with fewer than two read items in a language
has no floor and reads Regressed in every trial; that is printed.

## Steps

1. `weksa loop open ROOT ...` and `weksa loop round ROOT` record round N and
   leave its answers in `ROOT/round-N/`.
2. For each draw label: `weksa loop relower ROOT --round N --arm LABEL
   --lowerer NAME` writes the byte-identical lowering prompts to
   `ROOT/relower-N-LABEL/<digest>.prompt` and exits 3. Answer each prompt into
   `ROOT/relower-N-LABEL/<digest>.answer`.
3. `weksa loop calibrate ROOT --round N --prompted p1 --prompted p2 ...
   [--degraded LABEL]... [--candidate LABEL]... [--class NAME=PREFIX]...`.
   At least four `--prompted` labels are needed (a pair and three floors with
   `R` make five draws). It exits 1 and lists `unanswered LABEL COUNT` for any
   draw with prompts still unanswered. Labels match `[a-z0-9-]{1,32}` and differ
   across the three kinds. It needs the Ollama embedder of the run; a full
   60-item, 13-draw calibration is on the order of 1500 embedded texts, so
   launch it detached with a log.

The scratch packet store is `ROOT/calibrate-N/base.cc`; delete it at will.

## Answer file format

`<digest>.answer` is one JSON object and nothing else, no code fence:
`{"text": "..."}`, a JSON-escaped string. The answerer reads only the prompt
(its first line says what to write) and writes the answer. Every prompt of the
draw must be answered.

## Briefs per arm

Independence rule: a fresh agent per batch of answers; no batch sees another
draw's answers. Only `mild` reads `round-N/*.answer`.

- Plain prompted: answer each prompt as its own first line asks; read nothing
  else in the run root.
- Mild: for each prompt, read the recorded lowering in
  `ROOT/round-N/<digest>.answer` and write the same-format answer whose text is
  that utterance with exactly one meaning detail blurred: change or drop one of
  tense, polarity, modality or hedge, a referent, a quantity or a predicate's
  concept (replace it with a near but different word). Keep language, register,
  length and fluency; the result must read as a natural utterance and must not
  mention the change. Pick the detail that is not the most salient one. A
  second mild draw blurs a different detail than the first and is written
  without reading the first's answers. The wording of the distil-base original
  is lost, and the dispatch used for growth-base is not on disk, so this is the
  wording from now on. Warning: on growth-base two mild arms briefed this way
  came out identical in 27 of 60 items, so independently briefed mild arms
  are not independent degradations; read a mild draw's regress count with that
  in mind.
- Train-guide: before answering, read the Train sentences of the corpus in the
  batch's language as a register guide and let it steer word choice; the guide
  gives phrasing only and each answer must still say exactly what its own packet
  says. The guide section is the live Train items of that language, built from
  `weksa corpus dump` filtered to Train, and is given to the answerer in the
  brief, never pasted into this document, and never includes Validation or Test
  text. Read nothing else in the run root.
- Control: a script writes `{"text":"Yes."}` as the answer to every prompt.
