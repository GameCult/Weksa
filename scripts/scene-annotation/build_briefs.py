"""Build the two annotator briefs (rubric r1 section + one language's shuffled live
sentences), print the Test-leak count of each (must be 0) and its sha256.
Standard library only. Usage: from the repo root, WEKSA=... SCENE_SCRATCH=dir python3 -I scripts/scene-annotation/build_briefs.py"""
import re,sys,subprocess,random,json,hashlib,os
import os
W=os.environ.get("WEKSA","target/debug/weksa")  # the weksa binary
R=os.getcwd()  # run from the repo root
S=os.environ.get("SCENE_SCRATCH","scene-scratch")+"/"  # A-*.jsonl, B-*.jsonl, briefs; never committed
A=["--corpus","data/original/corpus.cc","--entities","data/original/entity.cc","--inventory","data/original/inventory.cc","--speaker-state","data/original/speaker-state.cc"]
out=subprocess.run([W]+A+["corpus","dump"],cwd=R,capture_output=True,text=True).stdout.split("\n")
live=[];test=[]
for i,l in enumerate(out):
    m=re.match(r'^item:(\S+) \[(\S+) \S+ group=(\S+) (\w+)\]$',l)
    if not m: continue
    t=out[i+1].strip()
    assert t.startswith('"') and t.endswith('"')
    t=json.loads(t) if '\\' in t else t[1:-1]
    if m.group(4)=="Test": test.append(t)
    else: live.append((m.group(2),m.group(3),t))
assert len(live)==98,len(live)
rub=open(R+"/docs/scene-annotation.md").read()
sec=rub[rub.index("## Rubric r1"):]
# rubric section ends at next "## " heading
rest=sec[len("## Rubric r1"):]
nx=rest.find("\n## ")
sec=sec[:len("## Rubric r1")+(nx if nx>=0 else len(rest))].strip()+"\n"
def leaks(brief,text):
    w=text.split(); c=list(text)
    return text in brief or any(" ".join(w[k:k+5]) in brief for k in range(len(w)-4)) or (len(w)==1 and any("".join(c[k:k+12]) in brief for k in range(len(c)-11)))
for lang in ("en-US","ja-JP"):
    rows=[(g,t) for (l,g,t) in live if l==lang]
    assert len(rows)==49
    random.Random(f"scene-r1-{lang}").shuffle(rows)
    body=sec+"\nThe groups, one per line as group<TAB>\"text\":\n\n"+"\n".join(f'{g}\t"{t}"' for g,t in rows)+"\n"
    n=sum(leaks(body,t) for t in test)
    print(lang,"lines",len(rows),"test_leaks",n,hashlib.sha256(body.encode()).hexdigest())
    open(S+f"brief-{lang}.txt","w").write(body)
