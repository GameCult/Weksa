# Tiny PanPhon v0.1 Speech Curriculum

This directory is the first executable Weksa-to-AquaSynth speech handoff
artifact.

It is intentionally small: six pure IPA packets for `a`, `pa`, `ta`, `ka`,
`sa`, and `ma`. Each packet uses
`weksa.utterance_embedding_handoff.v0.1` and carries:

- a 1024-float semantic text embedding reference, zeroed because these are pure
  IPA fixtures;
- an inline variable-length PanPhon 22-feature sequence;
- 32 Weksa-owned prosody/emphasis hints;
- a 64-float Epiphany-compatible neutral character-state vector reference;
- pending AquaSynth-owned 256-float phonetic realization and 64-float
  utterance embedding lanes.

The emitter is [tools/emit_tiny_panphon_handoffs.py](../../../tools/emit_tiny_panphon_handoffs.py).
It uses only the Python standard library and does not define a Weksa runtime.
The validator is [tools/validate_tiny_panphon_handoffs.py](../../../tools/validate_tiny_panphon_handoffs.py).
It checks the generated batch, packet refs, vector dimensions, PanPhon frame
shape, and pending AquaSynth-owned output lanes.

Run it from the Weksa repo root:

```powershell
py -3 tools\emit_tiny_panphon_handoffs.py
py -3 tools\validate_tiny_panphon_handoffs.py
```

`batch.json` is the training entrypoint for AquaSynth. The individual packet
files are stable seed fixtures. The schema uses PanPhon's 22-feature basis from
the start; the feature values in this six-phone crawl stage are hand-authored
and should be regenerated from the canonical PanPhon library before Weksa
expands coverage.
