# Cultural Ontology Profile Contract

This is the human-facing schema receipt for
`weksa.cultural_ontology_profile.v0`, the CultCache document family that stores
reviewed cultural ontology used during utterance lowering.

Executable schema code does not exist yet. When it does, it must reconcile with
this receipt, the meta-analysis paper, and the language ontology in
`data/ontology/ontology.cc`.

## Authority

A cultural ontology profile owns the realization lens for a target speech
community, dialect, register, regional overlay, ritual register, historical
period, or contact-language repertoire.

It owns:

- salience axes the lowerer must inspect
- social-indexical meanings attached to forms
- linguistic affordances available to a dialect or register
- activation gates and forbidden use contexts
- evidence, reviewer status, uncertainty, and source hashes
- trace requirements for lowering audits

It does not own:

- source meaning
- speaker identity
- final wording without an interlingua packet
- unreviewed stereotypes
- Qdrant nearest-neighbor results as accepted truth
- renderer-specific surface tricks

## State Shape

The profile is typed state whose nested fields are also typed state. Lists of
strings may be used only as summaries or indexes; accepted affordance authority
lives in feature records.

```yaml
schema: weksa.cultural_ontology_profile.v0
profile_id:
kind: base_language | dialect | sociolect | regional_overlay |
      institutional_register | religious_overlay | contact_language_overlay |
      historical_period_profile | community_of_practice
status: draft | draft_guarded | reviewed | deprecated

language:
  code:
  locale:
  script:
  variety_label:
  endonyms: []
  exonyms: []

parent_profiles: []

community:
  regions: []
  speech_community:
  historical_context:
  identity_authority:
  consent_notes:

activation:
  allowed_when: []
  forbidden_when: []
  required_confidence: exploratory | provisional | reviewed
  reviewer_required_when: []

salience_axes:
  <axis_id>:
    axis_id:
    domain: temporality_aspect | stance | social_relation |
            identity_indexing | sacred_or_ritual_context | address |
            modality | evidentiality | affect | politeness
    description:
    relevant_distinctions:
      - distinction_id:
        label:
        meaning:
        lowering_questions: []
        required_when: []
        forbidden_when: []
        evidence_refs: []

linguistic_affordances:
  grammar_features: []
  phonology_features: []
  lexicon_features: []
  discourse_patterns: []
  orthography_features: []
  code_switching:
    allowed:
    constraints: []
    patterns: []
```

## Affordance Feature Records

Every affordance list above contains typed feature records. Feature IDs should
be stable inside a profile and reusable in lowering traces.

```yaml
feature_id:
label:
status: draft | reviewed | deprecated
domain:
  layer: grammar | phonology | lexicon | discourse | orthography | code_switching
  category:
  subcategory:
description:
realization:
  function:
  surface_strategy:
  examples: []
activation:
  allowed_when: []
  forbidden_when: []
  requires_profile_authority:
  minimum_density:
  maximum_density:
constraints:
  grammar_contexts: []
  phonological_contexts: []
  register_contexts: []
  speaker_contexts: []
  collocation_limits: []
  incompatibilities: []
indexicality:
  social_meanings: []
  in_group_meanings: []
  out_group_risks: []
  stereotypes_to_avoid: []
evidence:
  source_refs: []
  qdrant_source_ids: []
  cached_source_refs: []
  source_hashes: []
  reviewer_refs: []
  confidence: low | medium | high
uncertainty:
  open_questions: []
  known_limits: []
trace:
  trace_label:
  must_report_when_used:
  must_report_when_rejected:
```

Specialized feature records add fields rather than replacing the common record:

```yaml
grammar_feature:
  feature_id:
  domain:
    layer: grammar
    category: tense_aspect_modality | negation | agreement | pronoun |
              word_order | subordination | discourse_particle
  formal_conditions:
    syntactic_environment: []
    semantic_inputs: []
    required_interlingua_fields: []
  lowering_effect:
    adds_meaning:
    preserves_meaning:
    changes_register:
    can_be_omitted_when: []

phonology_feature:
  feature_id:
  domain:
    layer: phonology
    category: vowel_shift | consonant_realization | prosody | rhythm |
              stress | intonation
  realization:
    ipa_hint:
    orthography_should_change:
    speech_synthesis_controls: []
  constraints:
    lexical_sets: []
    environments: []
    density_safe_for_text:

lexicon_feature:
  feature_id:
  domain:
    layer: lexicon
    category: regional_lexeme | loanword | calque | taboo | honorific |
              kinship | ritual_term | technical_term
  lexical_items:
    - form:
      gloss:
      sense:
      register:
      allowed_speaker_standing:
      forbidden_senses: []
      collocations: []

discourse_pattern:
  pattern_id:
  domain:
    layer: discourse
    category: stance_marker | turn_taking | address_sequence |
              narrative_frame | ritual_formula | humor | emphasis
  interactional_function:
  sequence_shape: []
  adjacency_constraints: []

orthography_feature:
  feature_id:
  domain:
    layer: orthography
    category: spelling_variant | script_choice | punctuation |
              transliteration | eye_dialect_guard
  allowed_surface_forms: []
  forbidden_surface_forms: []
  renderer_notes:

code_switching_pattern:
  pattern_id:
  domain:
    layer: code_switching
    category: lexical_insertion | tag_switch | alternation |
              quotation | ritual_formula | metalinguistic
  languages: []
  matrix_language:
  embedded_language:
  switch_sites: []
  pragmatic_functions: []
  constraints:
    requires_bilingual_speaker:
    requires_community_context:
    forbidden_when: []
```

## Lowering Trace Contract

A lowered utterance trace should cite the nested state that actually affected
the line:

```yaml
cultural_ontology_trace:
  activated_profiles: []
  activated_axes:
    - axis_id:
      distinctions_used: []
      distinctions_rejected: []
  activated_affordances:
    - profile_id:
      layer:
      feature_id:
      density:
      reason:
      evidence_refs: []
  rejected_affordances:
    - profile_id:
      layer:
      feature_id:
      reason:
  reviewer_required:
  uncertainty: []
```

## Storage

No profile store exists. The per-language lowering choices live in
`data/ontology/ontology.cc` (`weksa ontology dump`); the typed profiles that
once sat under `.weksa/cultural-ontology/`, and the `weksa.target_cultural_ontology.*`
base profiles, whose names an example below still carries, are deleted. Community and dialect
profiles are owned by follow-up `language-ontology-communities`.

Qdrant may store chunks, vectors, and payload filters for retrieval. It does
not own profile acceptance, feature activation, or renderer authority.
