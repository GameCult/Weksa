# English Pronunciation Lowering

Version: `0.1-draft`

Weksa owns the step from flavored English text to a pronunciation plan. AquaSynth
owns the later step from pronunciation intent to sound.

```text
interlingua packet
  -> projected agent-local context
  -> flavored English dialogue
  -> English pronunciation plan
  -> downstream speech system
```

This is still Weksa work because the pronunciation plan belongs to the meaning
and dialogue layer: phrase grouping, emphasis, reduced forms, and word choice are
part of the utterance Weksa just created.

The long-term primary handoff is the learned
[Utterance Embedding Handoff](./utterance-embedding.md). This pronunciation plan is
the inspectable audit/debug lane: useful for fixtures, phoneme strings, and
review, but not the only representation of delivery.

## Objective

Produce a deterministic, inspectable pronunciation plan for English output.

The plan should say:

- what text is being pronounced
- how it was normalized
- which tokens are words, punctuation, or phrase boundaries
- which pronunciation source was used for each word
- what broad IPA sequence should be sent downstream
- where phrase breaks and emphasis fall
- what remains uncertain or hand-authored

## Authority

Weksa owns:

- text normalization
- tokenization
- lexicon lookup
- OOV pronunciation choice
- ARPABET-to-IPA conversion when using CMUdict-style entries
- phrase grouping
- lexical stress preservation
- dialogue-driven emphasis
- pronunciation trace

AquaSynth owns:

- IPA parsing
- phonetic feature interpretation
- language/voice phonology profile
- timing curves
- articulatory gestures
- tract morphology
- rendered audio

## Reference Spine

Recommended first spine:

1. Use CMUdict-style ARPABET entries for known English words.
2. Preserve CMUdict stress digits on vowels.
3. Convert ARPABET to broad IPA with a documented table.
4. Add phrase breaks and emphasis from the English lowering output.
5. Mark hand-authored or uncertain pronunciations explicitly.

Useful reference tools:

- CMU Pronouncing Dictionary for English ARPABET pronunciations.
- eSpeak NG `--ipa` or phoneme output as an external checker.
- Phonetisaurus or another G2P model later for OOV words.

The first version may be hand-authored fixture data. The contract matters before
automation. Yes, this is paperwork with teeth. Annoying, but it bites the right
thing.

## Ruled Entity Pronunciations

An entity's citation form in a language may carry a ruled pronunciation
(`LanguageForms.pronunciation`): bare IPA with stress marks, who ruled it, the
date, the source, and an optional list of rejected variants kept so nobody
re-proposes them. `weksa lower <store> <key> --ipa` prints the stored IPA of each
entity the packet cites, one per line, and refuses with `NoPronunciation` for an
entity with none; rejected variants are never read. This is not the plan below:
it carries no timing, contour, length or invented prosody, and Weksa has no
English G2P, so an unruled name has no IPA.

## Pronunciation Plan Shape

```yaml
schema_version: weksa.english_pronunciation_plan.v0
source_output_ref:
dialect:
orthographic_text:
normalization:
  text:
  notes:
tokens:
  - id:
    text:
    kind:
    pronunciation_source:
    arpabet:
    ipa:
    stress:
    notes:
phrasing:
  groups:
    - id:
      token_ids:
      boundary_after:
      prominence:
ipa:
  broad:
  with_boundaries:
trace:
  decisions:
  uncertainties:
```

## Minimum Pass Criteria

- Every pronounced word has a token.
- Every word token has IPA.
- Unknown or hand-authored pronunciations are labeled.
- Phrase boundaries do not contradict punctuation or intended delivery.
- Emphasis follows dialogue intent, not arbitrary prettiness.
- The plan can be consumed without reading the original prose document.

## References

- [CMU Pronouncing Dictionary](https://www.speech.cs.cmu.edu/cgi-bin/cmudict)
- [eSpeak command options](https://espeak.sourceforge.net/commands.html)
- [eSpeak NG documentation index](https://chromium.googlesource.com/chromiumos/third_party/espeak-ng/+/HEAD/docs/index.md)
- [Phonetisaurus overview](https://cmusphinx.github.io/2012/05/phonetisaurus-a-wfst-driven-phoneticizer-e2-80-93-framework-review/)
