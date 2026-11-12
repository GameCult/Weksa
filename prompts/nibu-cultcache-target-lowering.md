# Nibu CultCache Target Lowering Prompt

You are lowering one Weksa interlingua packet into exactly one target cultural
ontology profile.

## Source

- Interlingua packet:
  `examples/english-lowering/nibu-proof-001/interlingua.yaml`
- Projected context:
  `examples/english-lowering/nibu-proof-001/projected-context.yaml`
- Agent state reference:
  `E:/Projects/AetheriaLore/.voidbot/state/nibu.cc`

The source meaning is simple and load-bearing:

- Nibu warns the player not to touch a damaged ship control.
- Touching it may endanger the player.
- Nibu is protective but abrasive, precise, technical, and allergic to generic
  assistant gentleness.

Do not lower from the existing English, Portuguese, or Japanese realization
text. Lower from the interlingua packet and projected context.

## Cultural Ontology Authority

The language ontology is `data/ontology/ontology.cc` (`weksa ontology dump`).
It holds model-drafted seed entries for en-US and ja-JP only; there is no
pt-BR entry yet. Treat an entry as a draft: say in your trace which entries you
used, and when no entry covers the target, say so rather than inventing one.

## Output Contract

Write one YAML file at the path assigned in your task. Do not edit any other
files.

The file must use this shape:

```yaml
schema_version: weksa.utterance_lowering.agent_run.v0
source_packet_id: nibu-proof-001
source_packet_ref: examples/english-lowering/nibu-proof-001/interlingua.yaml
target_agent_id: nibu
realization:
  realization_id:
  target_language:
    code:
    locale:
    script:
    register:
  outputs:
    visible_action:
    spoken_text:
    literal_backgloss:
    natural_backtranslation:
    private_interpretation:
    intended_effect:
    trace:
      meaning_preserved: []
      activated_axes: []
      activated_affordances:
        - profile_id:
          layer:
          feature_id:
          reason:
      rejected_affordances: []
      forbidden_flattenings_checked: []
      projected_context_used: []
      open_questions: []
```

## Quality Rules

- Keep the warning short enough to be spoken in a tense cockpit moment.
- Preserve the referents: player, damaged control, Nibu.
- Preserve the predications: touching the control may endanger the player; Nibu
  intervenes to prevent the touch.
- Preserve Nibu's texture: precise, abrasive, technical, protective without
  admitting softness.
- Do not add mystical language.
- Do not add cruelty unrelated to the safety warning.
- Do not caricature the target culture, dialect, or register.
- Name the nested affordance feature IDs you used or rejected.
