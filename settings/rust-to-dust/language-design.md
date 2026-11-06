# Language Design Pressure

Rust to Dust needs a language project whose speakers treat aesthetic devotion as
moral honor and cultural absorption as refinement rather than conquest. The
empire is not frightening because it is merely better at commodification. It is
frightening because its culture is overwhelmingly beautiful and humans are
capable of recognizing that beauty.

The language is not named yet. The speakers are not named yet. Naming should
preferably emerge from ontology, phonetics, and register rather than from an
English label chosen too early because the blank was annoying.

## Engine Boundary

This file describes pressure on the first language project. It must not become
generic Weksa architecture.

Weksa should support many ontologies. Rust to Dust happens to need one where
aesthetic virtue is central.

## Ontology Pillars

- aesthetic virtue
- performed mastery
- surface as evidence of inner order
- decorated versus unpainted social status
- violence as beautiful craft
- chosen constraint as proof of mastery
- overmatch as vulgar or tourist-coded
- vulgar violence as failed form
- commerce as a delivery rail for cultural gravity
- conquest as access and integration
- resistance as both disobedience and potential performance
- spectacle as a dangerous attractor for imperial interpretation
- local practice as meaning embedded in relationships
- human passion as evidence of aesthetic compatibility
- human refusal as admirable form
- cultural inheritance as continuity under aesthetic pressure

## Registers

Likely language-project registers:

- broadcast commentary
- high honor speech
- warrior discourse
- commercial acquisition language
- panel-show media language
- tourist/colonial admiration
- human resistance borrowing and misuse
- contaminated human English

## Runtime Data Needs

Structured intent carries neutral facts plus stance. The imperial ontology
derives form, discipline, spectacle and honor status from them downstream
(see `imperial/language-design.md`, Section 6).

Illustrative sketch (field names are not the packet schema; the executable
schema is `crates/weksa/src/packet.rs`). The client submits neutral facts and
stance only:

```json
{
  "domain": "combat",
  "act": "ambush",
  "actor": "father",
  "target": "imperial_guard",
  "instrument": "improvised_tool",
  "stance": { "affect": { "contempt": 600 }, "restraint": 800 }
}
```

An earlier draft of this example carried `"imperial_frame":
"authentic_local_defiance"`. That value is the imperial ontology's reading of
the facts above (performer-centered, local, sincere refusal), not a fact the
client can know, so it does not belong in the packet
(`weksa-body:ruling:rtd-imperial-aesthetic-reading-to-ontology`). Classes such as "authentic
local defiance" are outputs of the ontology projection.

## Open Questions

- What phonetic profile naturally fits this species and culture?
- Which distinctions does the language make obligatory?
- Which human distinctions does it collapse?
- How does scale decoration appear in metaphor, grammar, address, and insult?
- How does unpaintedness split across shame, danger, contempt, and purity?
- How does the language distinguish victory, mastery, spectacle, and vulgar
  overmatch?
- How does the language distinguish crude work from failed work?
- How does it encode respect for resistance that the speaker still expects to
  absorb?
- How does it distinguish public spectacle from living practice?
- How does imperial speech misread distributed cooperation when no central
  performer owns the act?
- What alien terms leak into human English first?
- How do subtitles balance player comprehension with imperial contamination?
