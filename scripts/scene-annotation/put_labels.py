"""Put annotator A's labels through weksa scene put, one revision per group.
Standard library only. Usage: as build_briefs.py."""
import json,subprocess
import os
W=os.environ.get("WEKSA","target/debug/weksa")  # the weksa binary
R=os.getcwd()  # run from the repo root
S=os.environ.get("SCENE_SCRATCH","scene-scratch")+"/"  # A-*.jsonl, B-*.jsonl, briefs; never committed
A=["--corpus","data/original/corpus.cc","--entities","data/original/entity.cc","--inventory","data/original/inventory.cc","--speaker-state","data/original/speaker-state.cc"]
rows=[json.loads(l) for f in ("A-en","A-ja") for l in open(S+f"{f}.jsonl")]
for r in sorted(rows,key=lambda r:r['group']):
    c=[W]+A+["scene","put","--group",r['group'],"--to","unnamed","--inventory-rev","4",
     "--set",f"relative_status={r['relative_status']}","--set",f"intimacy={r['intimacy']}","--set",f"in_group={'true' if r['in_group'] else 'false'}",
     "--model-family","anthropic","--generator","claude-sonnet-5-5 (Weksa Hands, scene rubric r1, docs/scene-annotation.md)"]
    p=subprocess.run(c,cwd=R,capture_output=True,text=True)
    print(r['group'],p.returncode,p.stdout.strip()[:80],p.stderr.strip()[:200])
