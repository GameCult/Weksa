# Roadmap

## Now

- Keep the persistent state spine small and readable.
- Stabilize the interlingua packet standard before runtime code.
- Prove interlingua lowering to human target-language realizations before
  attempting alien-language lowering, starting with English, `pt-BR`, and
  Japanese.
- Decide the first language-project state format after the interlingua boundary
  is clear.
- Build a hand-authored toy language-project slice before automating
  generation.
- Use Zyphos / Eusocial Interbeing as the current higher-priority proving
  ground while keeping Weksa setting-neutral. Rust to Dust remains an earlier
  pressure surface, not the active default.

## First Prototype

- Validate one minimal interlingua packet.
- Lower one packet through Nibu's projected agent state into English, Brazilian
  Portuguese, and Japanese realizations, each through an explicit target-language
  cultural ontology profile.
- Add one English pronunciation plan that strings the lowered line into broad
  IPA.
- Add one toy utterance embedding handoff fixture, then replace hand-authored
  values with the AquaSynth-owned learned encoder.
- Add a daemon loop that commits conversational intent, pronunciation plans, and
  utterance handoffs as typed CultCache witnesses.
- Publish the provider advertisement through CultMesh for Odin discovery; the
  operator view returns on Thing 2 (Eve is deprecated).
- Preserve the first English-only lowering proof under
  `examples/english-lowering/`, and the shared multilingual proof under
  `examples/multilingual-lowering/`.
- Preserve the Airawa first-contact slice as a hand-authored language-project
  pressure note before attempting full alien-language translation.
- Parse or load one language project definition.
- Accept a Weksa interlingua packet.
- Render glossed output through grammar, morphology, phonology, and diachrony.
- Emit a trace explaining which stage made each decision.
- Keep runtime LLM-free; model use belongs in authoring and critique.
- Keep CLI tools, fixtures, and daemon jobs on the same commit and
  derivation paths so Weksa does not grow split-brained intent truth.

## Later

- Add model-assisted semantic decomposition from English.
- Add embeddings for concept clustering.
- Add model critique against the explicit language state.
- Add corpus generation and consistency checks.
