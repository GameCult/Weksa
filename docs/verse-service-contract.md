# Weksa Verse Service Contract

> Status: planned contract. Nothing described below runs on main. A service is
> earned by cut `live-lowering` (see [the lowering map](./targets/interlingua-lowering-map.md));
> the Node body that once approximated it is parked at tag `parked/weksa-node-body`.

## Intended Authority

Weksa is intended to own conversational intent and utterance lowering: it will
turn interlingua packets (`weksa.interlingua_packet.v0`) and agent state
(`gamecult.persona_state.v0` or compatible projections) into typed
conversational intent, pronunciation plans and utterance handoff documents.

AquaSynth owns phonetic realization, synth-driver controls and rendered audio.
Speech providers own their audio bytes. Weksa will own only the request
projection and trace it hands them.

## Intended Publication

Weksa will publish its state as typed CultMesh documents over CultCache, with no
HTTP endpoint owning truth and no dashboard composition until Thing 2's surface
contract is ready (`thing2:ruling:eve-dashboards-quarantined-2`). The document
types and verse names are not fixed; they are decided when `live-lowering` is cut.

## Forbidden Writers

- Text renderers do not own meaning.
- AquaSynth does not own conversational intent.
- Dashboard renderers do not own Weksa state.
- Agent transcript memory cannot silently override typed Persona or intent
  documents.
