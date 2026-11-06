#!/usr/bin/env python3
"""Render proposals.json into one static HTML page.

usage: generate.py [proposals.json] [out/index.html]
Standard library only. The page has no scripts, no network calls and no data
beyond what proposals.json holds, so what is in the JSON is what gets published.
"""
import json
import pathlib
import sys
from html import escape as e

here = pathlib.Path(__file__).parent
src = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else here / "proposals.json"
out = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else here / "out" / "index.html"
data = json.loads(src.read_text(encoding="utf-8"))

CSS = """body{font:16px/1.55 system-ui,sans-serif;max-width:46rem;margin:2rem auto;padding:0 1rem;color:#1b1b1b;background:#fafafa}
h1{margin-bottom:.2rem}h2{margin:0 0 .3rem}.meta{color:#555;font-size:.9rem}
article{border:1px solid #bbb;border-radius:6px;padding:1rem 1.2rem;margin:1.5rem 0;background:#fff}
h3{margin:1rem 0 .2rem;font-size:1rem;text-transform:uppercase;letter-spacing:.04em;color:#333}
.ev h3{color:#0a6b4e}.inv h3{color:#a04a00}.forks li{margin:.4rem 0}
.stance{background:#f1f1f1;padding:.5rem .8rem;border-radius:4px}
nav a{margin-right:1rem}code{background:#eee;padding:0 .2rem}"""


def ul(items):
    return "<ul>" + "".join(f"<li>{e(i)}</li>" for i in items) + "</ul>"


def forks(p):
    if not p["forks"]:
        return ""
    items = "".join(f"<li>{e(f)}</li>" for f in p["forks"])
    return f'<div class="forks"><h3>Questions for you (one sentence each, in the project room)</h3><ol>{items}</ol></div>'


def card(p):
    ref = f"{p['repo']} #{p['pr']}" if p["pr"] else p["repo"]
    return f"""<article id="{e(p['id'])}">
<h2>{e(p['title'])}</h2>
<p class="meta">{e(ref)} · {e(p['status'])} · owner: {e(p['owner'])} · <a href="{e(p['url'])}">source on the forge</a></p>
<h3>What is proposed</h3><p>{e(p['what'])}</p>
<div class="ev"><h3>Rests on existing material</h3>{ul(p['evidence'])}</div>
<div class="inv"><h3>Weksa's invention</h3>{ul(p['invention'])}</div>
<h3>Weksa's stance and uncertainty</h3><p class="stance">{e(p['stance'])}</p>
{forks(p)}
</article>"""


page = f"""<!doctype html><html lang="en"><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Weksa proposals</title><style>{CSS}</style>
<h1>Weksa proposals</h1>
<p class="meta">State as of {e(data['as_of'])}. Nothing on this page is canon.</p>
<p>{e(data['intro'])}</p>
<nav>{"".join(f'<a href="#{e(p["id"])}">{e(p["title"].split(":")[0])}</a>' for p in data['proposals'])}</nav>
{"".join(card(p) for p in data['proposals'])}
</html>
"""
out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(page, encoding="utf-8")
print(f"wrote {out} ({len(data['proposals'])} proposals)")
