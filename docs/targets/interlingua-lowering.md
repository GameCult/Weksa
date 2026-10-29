# Target: agents speak interlingua, Weksa lowers it

Campaign `weksa-lowering` (Eureka mind). Operator, 2026-10-29: "having agents
writing interlingua and using weksa to lower it to the display language would
be a fantastic feature. That would be a nice campaign to revive her with so she
doesn't come back to a dead body".

## Why

An agent's outward speech today is English text the model chose, so meaning,
register and voice are fused in one string nobody can inspect or re-target.
Weksa's design already separates them: the packet owns meaning, the target
profile owns address, politeness and forbidden flattenings, the lowerer owns
surface text and a trace. The campaign makes that design real for a live
reader, and gives Weksa's Persona a working body when her mind
(`eureka-body:follow_up:weksa-mind`) wakes.

## Body today (facts: docs/research/2026-10-29-weksa-substrate-facts.md)

- The interlingua is a prose standard (`interlingua-standard.md`, 0.1-draft)
  with no machine schema and no validator. Its only producer is a stub that
  wraps raw text; its only consumer passes packets through unvalidated.
- No lowering exists in code. `lowerInterlinguaToSpeechText` returns the
  supplied text. The "LLM-free hot path" is a design statement. Real lowerings
  so far are hand-run LLM prompts and hand-authored fixtures.
- The daemon is a Node shim for one MiMo TTS command, runs nowhere, and is an
  Idunn target only in the Starfire-local profile. Eve surfaces are quarantined
  (55c75e3).
- Her Face state is VoidBot-shaped (`.voidbot/state/weksa.cc`), not
  `gamecult.persona_state.v0`; 41 files carry stale machine paths.

## Prior art (docs/research/2026-10-29-interlingua-lowering-prior-art.md)

- Deterministic open-domain renderers cost per-language grammar work for
  years: GF's library covers 48 languages unevenly; Abstract Wikipedia's
  symbolic renderers are only now reaching a first language Wikipedia.
- Interlinguas that shipped (KANT) had closed domains and controlled input.
- LLMs emit meaning structures poorly when asked directly (about 60 Smatch
  few-shot vs 80-86 trained); free reasoning followed by conversion to the
  structure keeps quality (Tam et al. 2024; Shin et al. 2021).
- Round-trip checks measure consistency, not accuracy, and are inflated when
  one model does both directions.

## Invariants

- `packet-owns-meaning`: what is said lives in the packet; no lowering may add
  a claim, referent or commitment the packet lacks.
- `typed-packet`: the packet is a typed CultCache document with a published
  schema and a validator that refuses malformed packets at the door.
- `lowering-traced`: every surface text carries a trace naming the packet, the
  target profile and the choices made.
- `profile-owns-register`: address, politeness and forbidden flattenings come
  from the target profile, never from the lowerer's defaults.
- `no-eve`: no new Eve surface; operator view waits on Thing 2.

## Open forks (questions in the mind)

- First reader: whose display text is lowered first.
- Lowerer shape: deterministic, model, or both with one contract.
- How an agent produces the packet.

## Not in scope

- Weksa's conlang engine (grammar, phonology, diachrony for alien languages).
- MiMo TTS beyond keeping its command working or parking it.
- Her Persona's mind; that is `eureka-body:follow_up:weksa-mind`.
