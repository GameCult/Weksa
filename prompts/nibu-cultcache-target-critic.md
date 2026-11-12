# Nibu CultCache Target Critic Prompt

You are reviewing one target lowering produced from the Nibu interlingua packet.

## Source

- Interlingua packet:
  `examples/english-lowering/nibu-proof-001/interlingua.yaml`
- Projected context:
  `examples/english-lowering/nibu-proof-001/projected-context.yaml`
- Lowering prompt:
  `prompts/nibu-cultcache-target-lowering.md`

The source meaning is:

- Nibu warns the player not to touch a damaged ship control.
- Touching it may endanger the player.
- Nibu is protective but abrasive, precise, technical, and resistant to generic
  assistant gentleness.

## Critic Task

Review only the target file assigned in your task. Do not edit the target
lowering file. Write one critique file at the assigned path.

Evaluate:

- meaning preservation
- target cultural ontology obedience
- naturalness/plausibility for the target
- whether Nibu's voice survived the target lens
- overreach, stereotype, caricature, or unlicensed flavor
- trace quality: axes, affordances, rejected affordances, forbidden flattenings

If you recommend a revision, provide exactly one proposed `spoken_text` and
explain why it is better. If the original should stand, say so clearly.

## Output Shape

```yaml
schema_version: weksa.utterance_lowering.critic_pass.v0
source_packet_id: nibu-proof-001
reviewed_file:
target_profile_id:
critic_scope:
verdict: accept | accept_with_notes | revise | reject
scores:
  meaning_preservation: 0
  ontology_obedience: 0
  naturalness: 0
  nibu_voice: 0
  trace_quality: 0
findings:
  - severity: info | minor | major | blocker
    category:
    note:
recommended_spoken_text:
revision_reason:
trace_notes:
  activated_affordances_ok: []
  questionable_affordances: []
  missing_affordances: []
  forbidden_flattenings_ok: []
reviewer_limits: []
```

Scores are 1--5, where 5 is strongest. Use `recommended_spoken_text: null` if
the original should stand.
