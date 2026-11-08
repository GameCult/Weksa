#!/usr/bin/env python3
"""Build the "Show her working" page from the real lowerer and scorer.

usage: build.py [out_dir]            (run from the repository root)

1. `cargo run --example show_working` admits each fixture packet through the
   packet store and lowers it with the English lowerer (crates/weksa/examples).
2. `weksa score` scores each lowered text against its corpus item (and the
   other committed Nibu reference) through the Ollama embedder.
3. out_dir/data.json holds the joined result; out_dir/index.html is one file
   with no scripts to fetch and no network calls.

Nothing on the page is typed by hand except the layout and the reading notes.
If cargo or the scorer cannot run, the build fails: there is no fallback text.
Standard library only.
"""
import json
import pathlib
import re
import subprocess
import sys
import time
from html import escape as e

root = pathlib.Path.cwd()
out_dir = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "tools/show-working/out")
weksa_bin = None


def run(cmd, **kw):
    r = subprocess.run(cmd, cwd=root, capture_output=True, text=True, **kw)
    if r.returncode != 0:
        sys.exit(f"{' '.join(map(str, cmd))} failed:\n{r.stderr}")
    return r.stdout


def cargo_target():
    meta = json.loads(run(["cargo", "metadata", "--format-version", "1", "--no-deps"]))
    return pathlib.Path(meta["target_directory"])


# -- 1. the real lowerer ------------------------------------------------------
run(["cargo", "build", "--release", "--bin", "weksa", "--example", "show_working"])
target = cargo_target() / "release"
weksa_bin = target / "weksa"
data = json.loads(run([str(target / "examples" / "show_working")]))

# -- 2. the real scorer -------------------------------------------------------
corpus = {}
for m in re.finditer(r'^(item:\S+) \[(.*?)\]\n  "(.*)"$', run([str(weksa_bin), "corpus", "dump"]), re.M):
    corpus[m.group(1)] = {"text": m.group(3), "meta": m.group(2)}
REFS_EXTRA = ["item:nibu:2"]
model = "qwen3-embedding:0.6b"


def score(text, items):
    out = run([str(weksa_bin), "score", *items], input=text)
    got = {}
    for m in re.finditer(r"^(item:\S+) chrf=([\d.]+) meaning=(-?[\d.]+)", out, re.M):
        got[m.group(1)] = {"chrf": float(m.group(2)), "meaning": float(m.group(3))}
    if set(got) != set(items):
        sys.exit(f"scorer answered {sorted(got)} for {items}:\n{out}")
    return got


for fx in data["fixtures"]:
    o = fx["outcome"]
    if o["kind"] == "lowered":
        refs = [fx["corpus_item"]] + [r for r in REFS_EXTRA if r != fx["corpus_item"]]
        o["scores"] = [{"item": k, "target": k == fx["corpus_item"], **v} for k, v in score(o["text"], refs).items()]

data["corpus"] = corpus
data["build"] = {
    "commit": run(["git", "rev-parse", "HEAD"]).strip(),
    "embedder": model,
    "built_unix": int(time.time()),
}
out_dir.mkdir(parents=True, exist_ok=True)
(out_dir / "data.json").write_text(json.dumps(data, indent=1, ensure_ascii=False), encoding="utf-8")

# -- 3. the page --------------------------------------------------------------
CSS = """
:root{--ink:#1c1b19;--mute:#5d5a54;--line:#cfc9bd;--bg:#f6f3ec;--card:#fff;--ok:#0b6b4b;--warn:#9a4a00;--bad:#9b1c2c;--bar:#365f9c}
*{box-sizing:border-box}body{font:16px/1.55 system-ui,sans-serif;max-width:62rem;margin:2rem auto;padding:0 1rem;color:var(--ink);background:var(--bg)}
h1{margin:0 0 .2rem}h2{margin:0 0 .4rem}h3{margin:1.2rem 0 .3rem;font-size:.85rem;text-transform:uppercase;letter-spacing:.06em;color:var(--mute)}
code,.mono{font:13px/1.4 ui-monospace,Menlo,monospace}code{background:#ece8dd;padding:0 .25rem;border-radius:3px}
.lede{max-width:46rem}.meta{color:var(--mute);font-size:.88rem}
nav.pick{display:flex;flex-wrap:wrap;gap:.4rem;margin:1.2rem 0}
nav.pick a{border:1px solid var(--line);background:var(--card);padding:.25rem .6rem;border-radius:999px;text-decoration:none;color:var(--ink);font-size:.9rem}
nav.pick a.refused{border-color:var(--bad)}nav.pick a.lowered{border-color:var(--ok)}
article{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:1rem 1.3rem;margin:1.4rem 0}
.chain{display:grid;grid-template-columns:repeat(4,1fr);gap:.6rem;margin:.6rem 0 1rem}
.chain div{border:1px solid var(--line);border-radius:6px;padding:.3rem .6rem;font-size:.82rem;color:var(--mute)}
.chain b{display:block;color:var(--ink)}
table{border-collapse:collapse;width:100%;font-size:.9rem}th,td{text-align:left;padding:.25rem .5rem;border-bottom:1px solid #e6e1d5;vertical-align:top}th{color:var(--mute);font-weight:600}
.out{font-size:1.15rem;padding:.6rem .9rem;border-left:4px solid var(--ok);background:#eef6f1}
.refusal{padding:.6rem .9rem;border-left:4px solid var(--bad);background:#faeef0}.refusal b{color:var(--bad)}
.tag{display:inline-block;font-size:.75rem;padding:0 .4rem;border-radius:3px;border:1px solid var(--line);color:var(--mute)}
.tag.read{border-color:var(--ok);color:var(--ok)}.tag.unread{border-color:var(--warn);color:var(--warn)}.tag.refuse{border-color:var(--bad);color:var(--bad)}
.bar{height:.7rem;background:#e4dfd2;border-radius:3px;min-width:8rem;position:relative}.bar i{display:block;height:100%;background:var(--bar);border-radius:3px}
.ref{font-size:.88rem;color:var(--mute);margin:.2rem 0 .6rem}.ref q{color:var(--ink)}
details{margin:.5rem 0}summary{cursor:pointer;color:var(--mute);font-size:.88rem}
section.gaps li{margin:.3rem 0}
@media(max-width:700px){.chain{grid-template-columns:1fr 1fr}}
"""

PICK_JS = """(function(){var a=[].slice.call(document.querySelectorAll('article[data-fx]'));function show(){var h=location.hash.slice(1);var any=a.some(function(x){return x.id===h});a.forEach(function(x){x.hidden=any&&x.id!==h})}
window.addEventListener('hashchange',show);show()})()"""


def g(chosen, i):
    return next((c["gloss"] for c in chosen if c["id"] == i), "?")


def val(v):
    (k, x), = v.items()
    return f"{x}‰" if k == "Grade" else str(x)


def slots_html(fx):
    s, ch = fx["slots"], fx["chosen"]
    rows = []
    for r in s["meaning"]["referents"]:
        ent = f" · entity <code>{e(r['entity'])}</code>" if r["entity"] else ""
        rows.append(f"<tr><td><code>{e(r['id'])}</code></td><td>referent</td><td>{e(g(ch, r['kind']))} / {e(g(ch, r['concept']))}{ent}</td></tr>")
    for p in s["meaning"]["predications"]:
        roles = ", ".join(f"{e(g(ch, x['role']))}=<code>{e(x['filler'])}</code>" for x in p["roles"])
        rows.append(f"<tr><td><code>{e(p['id'])}</code></td><td>predication</td><td>{e(g(ch, p['concept']))} in frame {e(g(ch, p['frame']))}: {roles}</td></tr>")
    for r in s["meaning"]["relations"]:
        rows.append(f"<tr><td></td><td>relation</td><td>{e(g(ch, r['relation']))}: <code>{e(r['from'])}</code> → <code>{e(r['to'])}</code></td></tr>")
    meaning = "<table><tr><th>node</th><th>kind</th><th>content</th></tr>" + "".join(rows) + "</table>"
    d = s["discourse"]
    drows = []
    if d["speech_act"]:
        drows.append(f"<tr><td>speech act</td><td>{e(g(ch, d['speech_act']))}</td></tr>")
    if d["addressee"]:
        drows.append(f"<tr><td>addressee</td><td><code>{e(d['addressee'])}</code></td></tr>")
    if d["addressee_consent"]:
        drows.append(f"<tr><td>addressee consent</td><td>predication <code>{e(d['addressee_consent'])}</code> (what the addressee asked for)</td></tr>")
    drows.append(f"<tr><td>source</td><td>{e(s['provenance']['source'])}, kind {e(s['kind'])}</td></tr>")
    return (f"<h3>Slot: meaning</h3>{meaning}<h3>Slot: discourse</h3><table>{''.join(drows)}</table>"
            + stance_register(fx))


def stance_register(fx):
    s = fx["slots"]
    eff = fx["outcome"].get("feature_effects") or []
    rows = []
    for i, f in enumerate(s["features"]):
        note = eff[i] if i < len(eff) else ""
        cls = "read" if "read it" in note else "unread" if "did not read" in note else "refuse" if "refus" in note else ""
        rows.append(f"<tr><td>{e(f['gloss'])} <span class='meta'>{e(f['id'])}</span></td><td>{e(f['slot'])} @ {e(f['at'])}</td><td>{e(val(f['value']))}</td><td><span class='tag {cls}'>{e(note)}</span></td></tr>")
    feats = ("<table><tr><th>feature</th><th>slot</th><th>value</th><th>measured effect on the lowerer</th></tr>" + "".join(rows) + "</table>") if rows else "<p class='meta'>No features: no hedge, certainty, polarity, irony or stance set.</p>"
    return ("<h3>Slot: stance, hedge, certainty, restraint (features)</h3>" + feats
            + "<p class='meta'>Register: the packet has no register slot. Register and address belong to a target-language profile, not to the packet; this lowerer reads none.</p>")


def outcome_html(fx):
    o = fx["outcome"]
    if o["kind"] == "lowered":
        ipa = o["ipa"]
        ipa_s = ("IPA of cited entities: <span class='mono'>" + e(" ".join(ipa["ok"])) + "</span>") if "ok" in ipa else f"IPA refused: {e(ipa['error']['reason'])} <code>{e(ipa['error']['variant'])}</code>"
        sc = []
        for s in o["scores"]:
            ref = corpus[s["item"]]["text"]
            sc.append(f"<div class='ref'>{'Target' if s['target'] else 'Other reference'} <code>{e(s['item'])}</code>: <q>{e(ref)}</q></div>"
                      f"<table><tr><td width=110>chrF {s['chrf']:.1f}</td><td><div class='bar'><i style='width:{min(s['chrf'],100):.0f}%'></i></div></td>"
                      f"<td width=150>meaning cosine {s['meaning']:.3f}</td><td><div class='bar'><i style='width:{max(s['meaning'],0)*100:.0f}%'></i></div></td></tr></table>")
        return (f"<h3>Lowered English</h3><div class='out'>{e(o['text'])}</div><p class='meta'>{ipa_s}</p>"
                f"<h3>Closeness to the corpus ({e(data['build']['embedder'])})</h3>{''.join(sc)}")
    if o["kind"] == "lower_error":
        er = o["error"]
        return f"<h3>Refused by the lowerer</h3><div class='refusal'><b>LowerError::{e(er['variant'])}</b>: {e(er['reason'])}<br><span class='mono'>{e(er['debug'])}</span></div><p class='meta'>No text, so no score: nothing was invented to score.</p>"
    items = "".join(f"<li><b>{e(r['rule'])}</b> <span class='mono'>{e(r['detail'])}</span></li>" for r in o["refusals"])
    return f"<h3>Refused at admission</h3><div class='refusal'><ul>{items}</ul></div><p class='meta'>The packet never became a lowering input, so the lowerer never saw it.</p>"


def card(fx):
    o = fx["outcome"]
    ch = "".join(f"<tr><td><code>{e(c['id'])}</code></td><td>{e(c['gloss'] or '(unresolved)')}</td></tr>" for c in fx["chosen"])
    steps = [("1 Typed slots", "meaning, stance, discourse"), ("2 Inventory entries", f"{len(fx['chosen'])} ids, rev {fx['slots']['inventory_rev']}"),
             ("3 Lowered English", {"lowered": "text", "lower_error": "refused", "admission_refused": "never reached"}[o["kind"]]),
             ("4 Closeness", "scored" if o["kind"] == "lowered" else "no text, no score")]
    chain = "".join(f"<div><b>{e(a)}</b>{e(b)}</div>" for a, b in steps)
    return (f"<article id='{e(fx['id'])}' data-fx><h2>{e(fx['title'])}</h2><p>{e(fx['note'])}</p>"
            f"<p class='meta'>Packet <code>{e(fx['packet_id'])}</code> for <code>{e(fx['corpus_item'])}</code>. Made from: {e(fx['source'])}.</p>"
            f"<div class='chain'>{chain}</div>{slots_html(fx)}"
            f"<details><summary>Inventory entries chosen ({len(fx['chosen'])})</summary><table><tr><th>id</th><th>gloss at this revision</th></tr>{ch}</table></details>"
            f"{outcome_html(fx)}</article>")


nav = "".join(f"<a class='{'lowered' if f['outcome']['kind']=='lowered' else 'refused'}' href='#{e(f['id'])}'>{e(f['title'])}</a>" for f in data["fixtures"])
b = data["build"]
page = f"""<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Weksa: show her working</title><style>{CSS}</style></head><body>
<h1>Show her working</h1>
<p class="lede">A meaning packet goes in. The English lowerer picks inventory entries, writes a sentence, and a scorer measures how close that sentence is to a human-written target. Every number and sentence below was produced by running the real Rust lowerer and scorer when this page was built. The refusals are results too: when the lowerer cannot say a meaning without changing it, it says so instead of dropping the part it cannot say.</p>
<p class="meta">Built from <code>{e(b['commit'][:10])}</code>, embedder <code>{e(b['embedder'])}</code>. English only. Fixtures are the committed <code>nibu-proof-001</code> packet and variants of it, each variant described in its own card. Scores are against the committed corpus: chrF (0-100) and embedding cosine. Raw data: <a href="data.json">data.json</a>.</p>
<nav class="pick">{nav}</nav>
{''.join(card(f) for f in data['fixtures'])}
<script>{PICK_JS}</script></body></html>"""
(out_dir / "index.html").write_text(page, encoding="utf-8")
print(f"wrote {out_dir}/index.html and data.json ({len(data['fixtures'])} fixtures)")
