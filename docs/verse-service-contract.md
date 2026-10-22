# Weksa Verse Service Contract

Weksa is the language-intent daemon. It turns writing, dialogue pressure, and
agent state into typed conversational intent, then lowers that intent into
utterance documents for speech, text, and synthesis consumers.

## Authority

Owner: Weksa.

Inputs:

- writing or dialogue drafts
- `weksa.interlingua_packet.v0` meaning packets
- `gamecult.persona_state.v0` or compatible agent-state projections
- language-project state, pronunciation rules, and flavor profiles
- optional speaker, scene, and discourse constraints

Outputs:

- `weksa.conversational_intent.v0`
- `weksa.pronunciation_plan.v0`
- `weksa.utterance_handoff.v0`
- `weksa.utterance_embedding_handoff.v0.1`
- `weksa.mimo_voicedesign_command.v0`
- `weksa.mimo_tts_request.v0`
- `weksa.mimo_voicedesign_receipt.v0`

Weksa owns conversational intent and utterance lowering. AquaSynth owns learned
phonetic realization, packed utterance embeddings, synth-driver controls, Faust
compilation, live instrument handles, and rendered audio. MiMo owns provider
voice rendering and audio bytes; Weksa owns the VoiceDesign request projection
and trace.

## CultCache Witnesses

- `.weksa/provider-advertisement.cc`: read-only provider advertisement witness.
- `.weksa/provider-advertisement-store.cc`: CultMesh/CultCache provider store
  containing `gamecult.eve.provider_advertisement.v1`,
  `weksa.operator_state.v0`, `weksa.command_boundary.v1`, and
  `weksa.transport_profile.v1` records for Odin discovery.
- `.weksa/operator-state.cc`: daemon health, readiness, and runtime witness
  index used by Idunn health checks and operator inspection.
- `.weksa/command-boundary.cc`: provider-owned command and lifecycle boundary
  naming Weksa as owner, Idunn as lifecycle requester, and the MiMo
  VoiceDesign RUDP command ingress as daemon-owned transport authority.
- `.weksa/transport-profile.cc`: target/current transport profile. Health is
  published through `cultnet.transport.rudp.v0`; provider state is carried by
  the CultMesh store; MiMo VoiceDesign command ingress now arrives through
  `cultnet.transport.rudp.v0` instead of an HTTP route.
- `.weksa/cultmesh-publications.cc`: publication index naming current Weksa
  witness surfaces and RUDP/CultMesh routes.
- `.weksa/intent/{intentId}.cc`: conversational intent documents.
- `.weksa/pronunciation/{utteranceId}.cc`: pronunciation plans and phonetic
  event sequences.
- `.weksa/handoff/{utteranceId}.cc`: utterance handoffs and embedding handoff
  requests for AquaSynth.
- `.weksa/speech-provider/mimo/{requestId}/command.cc`: daemon-owned RUDP
  command witness for MiMo VoiceDesign ingress.
- `.weksa/speech-provider/mimo/{requestId}/interlingua.cc`: accepted or
  provisionally decomposed interlingua used for a MiMo verbalization command.
- `.weksa/speech-provider/mimo/{requestId}/mimo-request.cc`: Weksa-owned MiMo
  VoiceDesign request projection.
- `.weksa/speech-provider/mimo/{requestId}/receipt.cc`: provider response
  receipt and artifact references.
- `.weksa/traces/{traceId}.cc`: lowering traces naming the stage that made each
  decision.
- `.weksa/projects/{projectId}.cc`: language-project and flavor-profile state.
- `.weksa/cultural-ontology/{storeId}.cc`: reviewed or draft guarded cultural
  ontology profile stores used by utterance lowering.

Early exporters may publish fixtures before a daemon loop exists, but the
witness names are not decorative. These are the state surfaces the daemon must
commit when the runtime lands.

The local Starfire daemon is launched by `scripts/start-weksa-daemon.ps1`,
checked by `scripts/health-weksa-daemon.cmd`, and restarted by
`scripts/restart-weksa-daemon.cmd`. Its daemon-owned health path publishes
`weksa.cultnet-rudp-provider-health` to Idunn over
`cultnet.transport.rudp.v0` after each serialized witness refresh, and MiMo
VoiceDesign commands arrive on `rudp://127.0.0.1:8813` as
`weksa.mimo_voicedesign_command.v0` documents. Health and operator truth live
in `.weksa/*.cc` witnesses; the HTTP daemon surface is deleted.

## CultMesh Verses

- `weksa.service`: service identity, schema catalog, version, and health.
- `weksa.intent`: conversational intent documents and review queues.
- `weksa.language_project`: language-project state, flavor profiles, and
  pronunciation rules.
- `weksa.cultural_ontology`: cultural ontology catalogs, profile stacks,
  salience axes, linguistic affordances, activation gates, and evidence refs.
- `weksa.pronunciation`: phonetic plans, IPA spans, prosody, and trace
  evidence.
- `weksa.utterance`: utterance handoff documents, packed embedding requests,
  and AquaSynth binding receipts.
- `weksa.speech_provider`: provider-specific request projections and receipts
  for external speech systems such as MiMo VoiceDesign.
- `weksa.operator`: daemon status, queue pressure, schema drift, and witness
  freshness.

Odin ingests the Starfire Weksa provider through
`.weksa/provider-advertisement-store.cc`. Idunn tracks the daemon as target
`weksa` in the `starfire-local` swarm profile and owns restart authority through
the Weksa restart script. Odin should read Weksa's provider advertisement,
operator state, command boundary, and transport profile from the
typed provider store and RUDP command boundary; no HTTP endpoint owns provider
truth.

## Operator Dashboards

Eve is deprecated, and Thing 2 is its successor (`thing2:ruling:eve-dashboards-quarantined`, 2026-10-22). Weksa's operator Eve surface publisher is quarantined: removed from main and the build, parked at the `parked/weksa-eve-surface` tag, with follow-up `thing2:follow_up:weksa-operator-view-on-thing2` to rebuild it on Thing 2. Until Thing 2's surface contract is ready, Weksa publishes typed CultMesh state (operator state, command boundary, transport profile, provider advertisement) and no dashboard composition.

## Commands

- `intent.create`: convert writing or interlingua into typed conversational
  intent.
- `intent.review`: accept, reject, or revise intent through the same commit
  path used by automated jobs.
- `utterance.lower`: lower accepted intent and agent state into pronunciation
  and utterance handoff documents.
- `handoff.export`: emit typed `.cc` witnesses for AquaSynth consumption.
- `speech_provider.mimo.voicedesign`: accept a Persona/agent state file plus
  either a Weksa interlingua packet or raw thought text. Raw thought is first
  decomposed into provisional interlingua, then lowered into a MiMo VoiceDesign
  request and rendered through MiMo. The daemon accepts this command as a
  `weksa.mimo_voicedesign_command.v0` document over `cultnet.transport.rudp.v0`.
- `provider.advertise`: publish `gamecult.eve.provider_advertisement.v1`.

## Forbidden Writers

- Text renderers do not own meaning.
- AquaSynth does not own conversational intent.
- Dashboard renderers do not own Weksa state.
- Fixture emitters cannot become an alternate runtime truth.
- Agent transcript memory cannot silently override typed Persona or intent
  documents.
- MiMo cannot rewrite accepted spoken text or become a second Persona-state
  interpreter.
