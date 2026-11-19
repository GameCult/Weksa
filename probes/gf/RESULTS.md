# GF probe: RGL application grammar, English + Japanese, Rust over the C runtime

Decision probe for option A (GF does the grammar, Weksa's Rust owns tree planning,
lexicon, phonology, coinage). Nothing here merges to main.
Run date 2026-11-19/20 on Yggdrasil (Debian 13 host, 16 cores, nice'd).

## Setup (what was installed, where)

- GF 3.12.0 from the official GitHub release `release-3.12`, asset
  `gf-3.12-ubuntu-24.04.deb` (9,072,270 bytes), installed with `dpkg -i` **inside** an
  `ubuntu:24.04` container image (`Dockerfile` here, tag `weksa-gf-probe:1`). Yggdrasil
  is Debian 13, not Ubuntu 24.04, so the host got no GF package at all.
- RGL: official asset `gf-rgl-20260403.zip` from the `GrammaticalFramework/gf-rgl`
  release 20260403, unzipped to `~/weksa-gf-probe/rgl`. It holds **compiled `.gfo` only,
  no `.gf` sources**.
- Rust 1.75 / cargo from Ubuntu apt inside the same image (not `ygg-verify.sh`'s rust
  image: that script is for repo verification and I needed GF's C libs beside cargo).
  Cargo has no dependencies; the crate links `libgu` and `libpgf` from the .deb.
- Host changes: one directory `~/weksa-gf-probe/` (rgl, dl, sources) and the docker image.

## Grammar

`grammar/`: abstract `Probe`, functor `ProbeI` (opens the RGL `Syntax` interface and
`LexProbe`), lexicon instances `LexProbeEng` / `LexProbeJpn`, concretes `ProbeEng` /
`ProbeJpn` = `ProbeI with (Syntax = SyntaxX) (LexProbe = LexProbeX)`. Constructors used:
`mkCl`, `mkNP`, `mkCN`, `mkAP`, `mkOrd`, `mkS negativePol`, `mkRS (mkRCl which_RP (mkVP can_VV (passiveVP v)))`, `mkUtt`.
Register is an abstract-level choice: `PhrPlain, PhrResp : Utt -> Phr`; each concrete
picks a cell of the RGL's `Utt` table through two lexicon opers (`plainStr`, `respStr`).

Trees (built by hand, no parser):

- a (ch. 8): `UttS (PosS (ClNPAP (TheCN (ModCN (SuperlA high_A) (UseN good_N))) (LikeAP like_A2 (BareN water_N))))`
- b (ch. 1): `UttS (NegS (ClNPNP (TheCN (RelCN (UseN way_N) (CanBePassedRS speak_V2))) (TheCN (ModCN (PosA constant_A) (UseN way_N)))))`

`-Utt` rows below linearize the bare RGL `Utt` (all table cells); `-Plain` / `-Resp` rows
wrap it in `PhrPlain` / `PhrResp`.

## Verbatim output of `bash probes/gf/run.sh`

```text
== compile grammar

real	0m0.196s
user	0m0.142s
sys	0m0.044s
-rw-r--r-- 1 ubuntu ubuntu 33960 Nov 19 23:04 Probe.pgf
== gf shell linearization (reference)
s : the highest good is like water
s Wa Plain : 一番 高い 善 は 水 の よう だ
s Wa Resp : 一番 高い 善 は 水 の よう です
s Ga Plain : 一番 高い 善 が 水 の よう だ
s Ga Resp : 一番 高い 善 が 水 の よう です

s : the way that can be spoken isn't the constant way
s Wa Plain : 言われる ことが できる 道 は 常の 道 ではない
s Wa Resp : 言われる ことが できる 道 は 常の 道 ではありません
s Ga Plain : 言われる ことが できる 道 が 常の 道 ではない
s Ga Resp : 言われる ことが できる 道 が 常の 道 ではありません

the highest good is like water
一番 高い 善 は 水 の よう だ

the highest good is like water
一番 高い 善 は 水 の よう です

== rust build
    Finished release [optimized] target(s) in 0.01s
-rwxr-xr-x 2 ubuntu ubuntu 13315048 Nov 19 23:03 target/release/gfprobe

real	0m0.030s
user	0m0.023s
sys	0m0.008s
== rust run
pgf_read_ms	1.337
tree	lang	field	string	n_parses	original_among	parse_trees
# tree a-Utt read back as: UttS (PosS (ClNPAP (TheCN (ModCN (SuperlA high_A) (UseN good_N))) (LikeAP like_A2 (BareN water_N))))
a-Utt	ProbeEng#v1	s	the highest good is like water	1	true	(original only)
a-Utt	ProbeJpn#v1	s Wa Plain	一番 高い 善 は 水 の よう だ	2	true	UttS (PosS (ClNPAP (TheCN (ModCN (SuperlA high_A) (UseN good_N))) (LikeAP like_A2 (BareN water_N)))) | UttS (PosS (ClNPAP (TheCN (ModCN (SuperlA high_A) (UseN good_N))) (LikeAP like_A2 (TheCN (UseN water_N)))))
a-Utt	ProbeJpn#v1	s Wa Resp	一番 高い 善 は 水 の よう です	0	false	 [parse raised]
a-Utt	ProbeJpn#v1	s Ga Plain	一番 高い 善 が 水 の よう だ	0	false	 [parse raised]
a-Utt	ProbeJpn#v1	s Ga Resp	一番 高い 善 が 水 の よう です	0	false	 [parse raised]
# tree a-Plain read back as: PhrPlain (UttS (PosS (ClNPAP (TheCN (ModCN (SuperlA high_A) (UseN good_N))) (LikeAP like_A2 (BareN water_N)))))
a-Plain	ProbeEng#v1	s	the highest good is like water	2	true	PhrPlain (UttS (PosS (ClNPAP (TheCN (ModCN (SuperlA high_A) (UseN good_N))) (LikeAP like_A2 (BareN water_N))))) | PhrResp (UttS (PosS (ClNPAP (TheCN (ModCN (SuperlA high_A) (UseN good_N))) (LikeAP like_A2 (BareN water_N)))))
a-Plain	ProbeJpn#v1	s	一番 高い 善 は 水 の よう だ	2	true	PhrPlain (UttS (PosS (ClNPAP (TheCN (ModCN (SuperlA high_A) (UseN good_N))) (LikeAP like_A2 (BareN water_N))))) | PhrPlain (UttS (PosS (ClNPAP (TheCN (ModCN (SuperlA high_A) (UseN good_N))) (LikeAP like_A2 (TheCN (UseN water_N))))))
# tree a-Resp read back as: PhrResp (UttS (PosS (ClNPAP (TheCN (ModCN (SuperlA high_A) (UseN good_N))) (LikeAP like_A2 (BareN water_N)))))
a-Resp	ProbeEng#v1	s	the highest good is like water	2	true	PhrPlain (UttS (PosS (ClNPAP (TheCN (ModCN (SuperlA high_A) (UseN good_N))) (LikeAP like_A2 (BareN water_N))))) | PhrResp (UttS (PosS (ClNPAP (TheCN (ModCN (SuperlA high_A) (UseN good_N))) (LikeAP like_A2 (BareN water_N)))))
a-Resp	ProbeJpn#v1	s	一番 高い 善 は 水 の よう です	2	true	PhrResp (UttS (PosS (ClNPAP (TheCN (ModCN (SuperlA high_A) (UseN good_N))) (LikeAP like_A2 (BareN water_N))))) | PhrResp (UttS (PosS (ClNPAP (TheCN (ModCN (SuperlA high_A) (UseN good_N))) (LikeAP like_A2 (TheCN (UseN water_N))))))
# tree b-Utt read back as: UttS (NegS (ClNPNP (TheCN (RelCN (UseN way_N) (CanBePassedRS speak_V2))) (TheCN (ModCN (PosA constant_A) (UseN way_N)))))
b-Utt	ProbeEng#v1	s	the way that can be spoken isn't the constant way	1	true	(original only)
b-Utt	ProbeJpn#v1	s Wa Plain	言われる ことが できる 道 は 常の 道 ではない	1	true	(original only)
b-Utt	ProbeJpn#v1	s Wa Resp	言われる ことが できる 道 は 常の 道 ではありません	0	false	 [parse raised]
b-Utt	ProbeJpn#v1	s Ga Plain	言われる ことが できる 道 が 常の 道 ではない	0	false	 [parse raised]
b-Utt	ProbeJpn#v1	s Ga Resp	言われる ことが できる 道 が 常の 道 ではありません	0	false	 [parse raised]
# tree b-Plain read back as: PhrPlain (UttS (NegS (ClNPNP (TheCN (RelCN (UseN way_N) (CanBePassedRS speak_V2))) (TheCN (ModCN (PosA constant_A) (UseN way_N))))))
b-Plain	ProbeEng#v1	s	the way that can be spoken isn't the constant way	2	true	PhrPlain (UttS (NegS (ClNPNP (TheCN (RelCN (UseN way_N) (CanBePassedRS speak_V2))) (TheCN (ModCN (PosA constant_A) (UseN way_N)))))) | PhrResp (UttS (NegS (ClNPNP (TheCN (RelCN (UseN way_N) (CanBePassedRS speak_V2))) (TheCN (ModCN (PosA constant_A) (UseN way_N))))))
b-Plain	ProbeJpn#v1	s	言われる ことが できる 道 は 常の 道 ではない	1	true	(original only)
# tree b-Resp read back as: PhrResp (UttS (NegS (ClNPNP (TheCN (RelCN (UseN way_N) (CanBePassedRS speak_V2))) (TheCN (ModCN (PosA constant_A) (UseN way_N))))))
b-Resp	ProbeEng#v1	s	the way that can be spoken isn't the constant way	2	true	PhrPlain (UttS (NegS (ClNPNP (TheCN (RelCN (UseN way_N) (CanBePassedRS speak_V2))) (TheCN (ModCN (PosA constant_A) (UseN way_N)))))) | PhrResp (UttS (NegS (ClNPNP (TheCN (RelCN (UseN way_N) (CanBePassedRS speak_V2))) (TheCN (ModCN (PosA constant_A) (UseN way_N))))))
b-Resp	ProbeJpn#v1	s	言われる ことが できる 道 は 常の 道 ではありません	1	true	(original only)
```

## Outputs

| tree | English | Japanese Plain | Japanese Resp |
|---|---|---|---|
| a | the highest good is like water | 一番 高い 善 は 水 の よう だ | 一番 高い 善 は 水 の よう です |
| b | the way that can be spoken isn't the constant way | 言われる ことが できる 道 は 常の 道 ではない | 言われる ことが できる 道 は 常の 道 ではありません |

Japanese output is space-separated at every token (GF tokenization); the parser needs
the spaced form, so a surface renderer must strip spaces after the round trip.
The RGL table also carries a topic/subject axis: `s Ga Plain` / `s Ga Resp` give the same
sentences with が for は.

## Parse-back (same language, parse category Utt for -Utt rows, Phr for the others)

| tree row | lang | cell | parses | original among |
|---|---|---|---|---|
| a-Utt | Eng | s | 1 | yes |
| a-Utt | Jpn | Wa Plain | 2 | yes |
| a-Utt | Jpn | Wa Resp, Ga Plain, Ga Resp | 0 (parser raises) | no |
| a-Plain | Eng | s | 2 | yes |
| a-Plain | Jpn | s | 2 | yes |
| a-Resp | Eng | s | 2 | yes |
| a-Resp | Jpn | s | 2 | yes |
| b-Utt | Eng | s | 1 | yes |
| b-Utt | Jpn | Wa Plain | 1 | yes |
| b-Utt | Jpn | Wa Resp, Ga Plain, Ga Resp | 0 (parser raises) | no |
| b-Plain | Eng | s | 2 | yes |
| b-Plain | Jpn | s | 1 | yes |
| b-Resp | Eng | s | 2 | yes |
| b-Resp | Jpn | s | 1 | yes |

Why the extras:

- Phr rows, English: `PhrPlain` and `PhrResp` render identically in English, so the
  register is unrecoverable from the string (2 parses, differing only in the head).
- a, Japanese: the second parse is `TheCN (UseN water_N)` for `BareN water_N`. Japanese
  has no article, so definiteness is not recoverable. A genuine ambiguity of the language,
  not a grammar bug.
- Non-first Utt cells: the C runtime parser parses field 0 of a category only. A
  `Style`/`Particle` cell other than `Wa Plain` cannot be parsed back from `Utt`; the
  `Phr` wrapper is the workaround that makes polite text parsable.

## Measurements (Yggdrasil, nice'd, 16 cores)

| item | value |
|---|---|
| GF .deb | 9.07 MB; dpkg Installed-Size 62,728 KB |
| `gf` binary | 15.3 MB; `libpgf.so` 0.20 MB, `libgu.so` 0.24 MB, `libpgf.a` 0.34 MB, headers 0.13 MB |
| RGL compiled `.gfo` | zip 27.8 MB; unpacked 208 MB (alltenses 55 MB, present 27 MB, rest morphodict); the Eng+Jpn+shared `Syntax/Paradigms/Lang` subset is about 5.7 MB |
| Dockerfile image `weksa-gf-probe:1` | 1.13 GB as `docker images` lists it, almost all Ubuntu cargo/rustc/gcc |
| grammar compile (`gf -make ProbeEng.gf ProbeJpn.gf`, cold, no .gfo cache) | 0.2 s wall |
| `Probe.pgf` | 33,960 bytes |
| Rust crate build (clean, release, no dependencies) | 0.58 s wall; binary 13.3 MB |
| PGF load in Rust | 1.3 to 2.1 ms |
| whole Rust run (load, 6 trees x 2 langs, linearize, 18 parses) | 9 ms wall |

Build host and target: all builds ran on Yggdrasil (Linux x86_64, container Ubuntu 24.04),
which is also the only platform proven. Nothing was built or run on the Windows workstation.

## Walls hit

1. **Host OS.** The release .deb is Ubuntu 24.04; Yggdrasil is Debian 13. Used a container.
2. **RGL release zip has no sources.** `i -retain ParadigmsJpn` fails ("no source
   grammar in scope"). Application grammars compile against the .gfo fine, but the Japanese
   API signatures had to be found from type errors (`mkV2 : Str -> V2`, `mkA2 : Str -> Str -> A2`;
   not `mkA2 : A -> Prep -> A2` as in English). Reading `ResJpn.gf` needs a git clone.
3. **Table access.** `pgf_linearize` emits field 0 only. The `Style` cells come out
   through `pgf_lzr_concretize` + `pgf_lzr_get_table` + `pgf_lzr_linearize_simple`, which are
   in `linearizer.h` but not in the runtime-api page. Plain `extern "C"` was enough; the one
   thing the header hides is `gu_exn_clear`, a static inline, which I reproduced by writing
   the first two fields of `GuExn` (state, caught). That couples the crate to the struct layout.
4. **Parser covers field 0 only** (see above). Register on the parse side needs the
   abstract-level `PhrPlain`/`PhrResp` encoding.
5. **FFI traps.** `gu_string_in` does not copy its string: a temporary `CString` gave
   garbage trees and then `pgf_print_expr (pgf/expr.c:1539): assertion failed`, which aborts the
   process. The C runtime asserts rather than raising on invalid expressions.
6. **English contraction.** `mkS negativePol` linearizes "isn't". The uncontracted form is
   in the English `Extra` module; not pursued.
7. **Japanese spaces.** Token-separated output needs post-processing.
8. **Politeness is two-valued.** Only `Plain`/`Resp` (and the Wa/Ga particle). No lexical
   keigo; no way to say sonkeigo versus kenjougo.

## Japanese quality (non-native judgement)

- a: 一番高い善は水のようだ. Grammatical and parseable by me. 一番 is colloquial
  ("the most"), 最も would fit a classical text better; 善 bare is fine. Polite ようです is natural.
- b: 言われることができる道は常の道ではない. Understandable and grammatical, but
  stilted: passive plus ことができる ("can be said") where a native writer would use 言える
  or 語りうる. 常の道 is my lexical guess for "constant", not an idiomatic compound (常道
  would read better). The RGL has no potential-form construction that maps to "can be
  spoken"; the modal composes mechanically. Polite ではありません is correct.
- Judged only by reading; no native speaker checked it.

## Reproduce

`bash probes/gf/run.sh` (syncs `probes/gf` to `~/weksa-gf-probe/probes/gf` on Yggdrasil,
compiles the PGF, prints the GF shell reference lines, builds the crate, runs it). Needs the
image built from `Dockerfile` (the .deb copied to `dl/` first, ignored by git) and the unzipped
RGL at `~/weksa-gf-probe/rgl`.
