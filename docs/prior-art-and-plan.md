# Prior art and plan

Meta asked (#weksa, 2026-11-13) for scaffolding to stand on instead of
building from base principles. This is a survey of what exists and a plan that
borrows it. Every source below was read in full text unless marked **snippet**
(seen only in a search result) or **not read**. Research only: nothing here is
built, and no new loop machinery is proposed.

## What exists

**Semantic backbones: none carries the part we care about.**

- AMR ([Banarescu 2013](https://aclanthology.org/W13-2322.pdf)): no politeness,
  speech-act or pragmatics text at all; it also does not distinguish real from
  hypothetical events (its Section 4).
- UMR ([Van Gysel 2021](https://par.nsf.gov/servlets/purl/10288269)): adds
  document-level coreference, temporal and modal-dependency graphs. The paper
  has no politeness, addressee or register layer; the only speaker-like object
  is the author node. Annotation agreement was still "awaiting confirmation".
- Universal Dependencies: the `Polite` feature has four values (`Infm`, `Form`,
  `Elev`, `Humb`; [feature page](https://universaldependencies.org/u/feat/Polite.html)),
  but the Japanese GSD treebank carries no `Polite`
  ([treebank page](https://universaldependencies.org/treebanks/ja_gsd/index.html)).
  The page says Japanese *teineigo* shares the value `Form` with formal register
  although it is a different axis. UD is a parse scheme, not a meaning scheme.
- Grammatical Framework RGL: 38 languages, Japanese present
  ([synopsis](https://www.grammaticalframework.org/lib/doc/synopsis/index.html),
  [status](https://www.grammaticalframework.org/lib/doc/status.html)); the source
  declares `Style = Plain | Resp`, so two politeness levels and no honorific
  paradigm
  ([ResJpn.gf](https://raw.githubusercontent.com/GrammaticalFramework/gf-rgl/master/src/japanese/ResJpn.gf)).
  LGPL/BSD, application grammars may take any licence
  ([LICENSE](https://raw.githubusercontent.com/GrammaticalFramework/gf-rgl/master/LICENSE)).
- Abstract Wikipedia / Wikifunctions: encyclopedic content, 2026 soft launch in
  three languages, no politeness
  ([Meta page](https://meta.wikimedia.org/wiki/Abstract_Wikipedia)). The
  Wikifunctions NLG and Japanese documentation were **not read**.

**Typology.**

- WALS (CC BY 4.0, CLDF at <https://github.com/cldf-datasets/wals>): the only
  politeness feature is 45A, pronoun politeness
  ([chapter](https://wals.info/chapter/45)); Japanese is coded value 4 ([datapoint](https://wals.info/valuesets/45A-jpn)), pronoun
  avoidance. No feature for verbal honorifics or register.
- Grambank (CC BY 4.0, <https://github.com/grambank/grambank>): GB415 is the
  second-person politeness distinction; I found no verbal-honorific feature
  (loose name match, not exhaustive). The 195-feature / 2,467-variety size is
  **snippet**; Japanese's GB415 value was **not looked up**.

**Formality control in MT: the closest prior art, and it is a sentence-level
label.**

- [Sennrich 2016](https://aclanthology.org/N16-1005.pdf): a side-constraint
  token controls T/V; forced-polite output was 96% polite or neutral.
- [Feely 2019](https://aclanthology.org/D19-5203.pdf): the same for EN-JA with
  three levels (informal, polite, formal); labels by rule plus a fastText
  classifier (F1 0.83-0.86 on 150 hand-labelled sentences); formal text is only
  0.7-1.6% of training data.
- [CoCoA-MT](https://arxiv.org/pdf/2205.04022) (CDLA-Sharing-1.0, repo
  <https://github.com/amazon-science/contrastive-controlled-mt>): 1,000 train and
  600 test segments for Japanese with contrastive formal/informal references and
  phrase-level markers. Two baseline systems' EN-JA outputs, labelled by
  professionals, came out formal 29.0% / 73.8% and "other" about 20-24%; EN-JA
  inter-annotator agreement 0.82, the lowest of the six languages.
- [Marrese-Taylor 2023](https://aclanthology.org/2023.wmt-1.49.pdf): GPT-3 and
  ChatGPT prompted reach about 90% formality-control accuracy on EN-JA, which
  the authors say beats fine-tuned MT; classifier and data at
  <https://github.com/epochx/japanese-formality> (no licence reported).
- [IWSLT 2023 formality track](https://aclanthology.org/2023.iwslt-1.1.pdf): no
  Japanese; metrics matched-accuracy (phrase markers) and classifier-accuracy.
  Niu 2017 is read ([D17-1299](https://aclanthology.org/D17-1299.pdf)); Niu 2019
  and Rippeth 2022 (arXiv 2205.06644, abstract only) are **not read**.
- None of these takes a speaker-state or addressee relation. The label is one
  bit or three levels attached to a source sentence.

**Grammar-in-context (for conlang targets).**

- [MTOB](https://arxiv.org/pdf/2309.16575) (MIT code, CC BY 4.0 grammar book):
  Claude 2 with word list plus retrieved sentences plus long grammar 44.7 / 45.8
  chrF against a human learner at 51.6 / 57.0.
- [Aycock 2025](https://arxiv.org/pdf/2409.19151): most of the gain comes
  from the book's parallel examples (Gemini-1.5-Flash eng->kgv chrF++ 30.8 with
  examples only against 34.4 whole book and 11.0 zero-shot; explanations alone
  22.6). Kalamang is a natural language, so transfer to a conlang is assumed.
- [Gemini 1.5 report](https://arxiv.org/pdf/2403.05530): full book 5.46 against a
  human learner's 5.52 on a 0-6 human scale.
  Suggests, for a conlang target (assumed to transfer), worked paired examples first and grammar
  prose second.

**Prompt optimisation and evaluation.**

- [DSPy](https://arxiv.org/pdf/2310.03714) (MIT), [MIPRO](https://arxiv.org/pdf/2406.11695),
  [OPRO](https://arxiv.org/pdf/2309.03409) (Apache-2.0): DSPy compiles take
  "10-20 trials over 150-300 validation examples"; MIPRO's task sets run from 75
  train (Iris) to 500 per task. MIPRO found bootstrapped demos gave the biggest
  gain and tests with Wilcoxon over 5 runs. OPRO discusses overfitting to the
  training set.
- [Koehn 2004](https://aclanthology.org/W04-3250.pdf): paired bootstrap;
  against a 30,000-sentence reference, a 2.0-point BLEU gap was detected at 95%
  significance in 31% of runs at n=100 and 65% at n=300; a 5.1-point gap in 97%
  at n=100. The paper suggests n=100 might be too small.
- [Dror 2018](https://aclanthology.org/P18-1128.pdf): paired bootstrap is
  "less effective for small test sets"; approximate randomization has been used
  for test sets above 20 points; only 3 of 110 surveyed ACL 2017 papers
  corrected for multiple comparisons.
- [Kocmi 2021](https://arxiv.org/pdf/2107.10821): pairwise accuracy against
  humans COMET 83.4, BLEURT 80.0, chrF 75.6, BLEU 74.6; recommends COMET
  primary, a string metric secondary and a paired test; observes "a slight drop
  in metric ranks for some pretrained metric" against ChrF for non-Latin and
  logogram targets and says more analysis would be required.
- [chrF](https://aclanthology.org/W15-3049.pdf): validated on seven targets, Japanese
  not among them.

## What we borrow

| Need | Borrow | Instead of |
|---|---|---|
| Politeness vocabulary for the packet's register field | UD `Polite` values (Infm, Form, Elev, Humb) as the label set, and Feely's three-level Japanese split | inventing level names |
| Modal/author structure | UMR's conceiver and modal-dependency idea for evidence/stance fields | a bespoke modality scheme |
| Japanese test of register | CoCoA-MT JA set (600 contrastive test segments, phrase markers), KeiCO for relation-labelled Japanese, and the Japanese-formality classifier | hand-written register references only |
| Conlang scaffolding | MTOB recipe: worked paired examples first, word list, grammar prose after (shown on Kalamang, a natural language; transfer to a conlang is an assumption) | long grammar prompts alone |
| Typology priors | WALS 45A and Grambank GB415 to seed which targets have a politeness axis | per-language guesses |
| Significance | paired approximate randomization over items; chrF with an embedding metric | reading small differences as results |
| Optimiser | DSPy/MIPRO only once a scoring set of at least 75 items exists (MIPRO's smallest task), more for stable compiles; the 35-item sets are below that | the golf loop growing a private optimiser |

We do not adopt AMR, UMR, UD or GF as the packet. None represents the speaker's
relation to the addressee, which the packet's scene state does. The packet stays
ours; it borrows vocabulary.

## What is actually new

Prior work already conditions honorific choice on the speaker-addressee
relation or status in narrower forms: KeiCO (Liu and Kobayashi 2022, 10,007
examples, annotates the speaker-listener social relationship; read through
Marrese-Taylor 2023 and the abstract), Korean context-aware NMT that infers the
relation from surrounding sentences
([MDPI](https://www.mdpi.com/2079-9292/10/13/1589), snippet), Brown-Levinson
generation that takes power and distance as parameters (Walker et al., not
read), and a 2026 translator given a known addressee
([Cihlar et al.](https://aclanthology.org/2026.iaai-1.3), snippet only). What I
did not find is the whole chain: persona state (status, restraint, stance) to a
register decision for an LLM lowerer, measured against a prompted baseline.
That is the claim, and it is relative to what I searched; closer work may
exist.

Uncontrolled commercial MT left 20-24% of EN-JA outputs inconsistent in
register (CoCoA-MT Table 4, two generic systems, no formality instruction). That
shows register is unstable without control, not how a prompted LLM behaves.

## Smallest end-to-end slice

One path, existing machinery, ja-JP only, no new loop:

1. A packet with scene state (speaker, addressee, relation) goes through the
   current `weksa loop` encode and lowering to Japanese.
2. The same packet under two relations (to a superior and to an intimate) must
   produce the two UD `Polite` levels; check with the Japanese formality
   classifier plus a native-reader pass on the 35-item Validation corpus per
   language (see the map's recalibration blocks).
3. Score the register decision as a classification (matched accuracy), not only
   chrF. CoCoA-MT's JA references give an outside check on the classifier, a
   weak one: its reference-based segment classifier on EN-JA had precision
   0.71 and 0.69 and recall 0.43 and 0.54 in the paper's Table 5.

Out of scope for the slice: conlang targets, optimisers, a second language.

## What good enough means

- Register accuracy: report matched accuracy for ours and for a plain prompt
  that is given the relation in words, on the same items, with paired
  intervals. We measure that baseline ourselves; Marrese-Taylor's roughly 90%
  is classifier accuracy (0.83-0.86 by phrase-marker matching) for binary
  formal/informal control with the label named in the prompt, an upper-region
  reference and not our baseline. The claim is non-inferiority with a stated
  margin. Superiority is claimed only if enough items are discordant (about 6
  or more, all in one direction, at n=35); against a baseline near 90%, n=35
  cannot show it, and the result is then "not detected".
- Meaning is not lost: chrF and meaning cosine are not detectably below the
  plain prompt on the same items, under the same paired test.
- Honest sample size: 35 items per language detects only large gaps, so results
  carry intervals and "not detected" stands as an outcome. Small gaps are not
  claims. Koehn's figures are for sentence-level BLEU on 100-3000-sentence
  blocks and do not transfer to a binary register metric on 35 items; power
  for that is computed directly from the discordant-pair count.
- A native-reader check on register for a sample, because every automatic metric
  here was validated on languages other than the persona register we care about.

## Open

- Which persona fields (status, restraint, stance) are in the packet today
  versus planned: this doc does not decide that; `docs/interlingua-standard.md`
  owns it.
- Licences not checked: the UMR dataset, the other four Japanese UD treebanks,
  CoCoA-MT repo licence ambiguity (paper says CDLA-Sharing-1.0, GitHub says
  NOASSERTION).
