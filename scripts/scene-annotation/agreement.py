"""Agreement of annotator A, B, the older labels and the round-1 encoder. Run it BEFORE the
rubric r1 revisions are put: the older labels are the store's latest revision then.
Standard library only. Usage: as build_briefs.py, plus ROUND1=<round-1 dir of the run>."""
import re,sys,subprocess,json,glob,os
import os
W=os.environ.get("WEKSA","target/debug/weksa")  # the weksa binary
R=os.getcwd()  # run from the repo root
S=os.environ.get("SCENE_SCRATCH","scene-scratch")+"/"  # A-*.jsonl, B-*.jsonl, briefs; never committed
A=["--corpus","data/original/corpus.cc","--entities","data/original/entity.cc","--inventory","data/original/inventory.cc","--speaker-state","data/original/speaker-state.cc"]
def run(*a): return subprocess.run([W]+A+list(a),cwd=R,capture_output=True,text=True).stdout.split("\n")
out=run("corpus","dump")
items=[]
for i,l in enumerate(out):
    m=re.match(r'^item:(\S+) \[(\S+) \S+ group=(\S+) (\w+)\]$',l)
    if m and m.group(4)!="Test":
        t=out[i+1].strip(); t=json.loads(t) if '\\' in t else t[1:-1]
        items.append(dict(id=m.group(1),lang=m.group(2),group=m.group(3),text=t,new=m.group(1).startswith("vgrow")))
G={x['group']:x for x in items}
assert len(G)==98
# scene dump: latest and @1 per group
sd=run("scene","dump"); lab={}
for i,l in enumerate(sd):
    m=re.match(r'^scene:(\S+)@(\d+) rev:\d+ Model \S+ (.*) \[entity:keeper\]$',l)
    if m:
        f=re.search(r'relative_status=(\w+) intimacy=(\d+) in_group=(\w+)',sd[i+1])
        lab.setdefault(m.group(1),{})[int(m.group(2))]=dict(rs=f.group(1),im=int(f.group(2)),ig=f.group(3)=="true",gen=m.group(3))
old={g:v[max(v)] for g,v in lab.items()}
def load(f): return {json.loads(l)['group']:json.loads(l) for l in open(S+f+".jsonl")}
Aa={**load("A-en"),**load("A-ja")}; Bb={**load("B-en"),**load("B-ja")}
assert set(Aa)==set(G)==set(Bb)
def kappa(x,y):
    n=len(x); 
    if n==0: return float('nan')
    po=sum(a==b for a,b in zip(x,y))/n
    pa=sum(x)/n; pb=sum(y)/n
    pe=pa*pb+(1-pa)*(1-pb)
    return float('nan') if pe==1 else (po-pe)/(1-pe)
def tab(x,y):
    return "[TT=%d TF=%d FT=%d FF=%d]"%(sum(a and b for a,b in zip(x,y)),sum(a and not b for a,b in zip(x,y)),sum((not a) and b for a,b in zip(x,y)),sum((not a) and (not b) for a,b in zip(x,y)))
def cmp(name,P,Q,gs,bkey=None):
    # P,Q: dict group-> record with keys addressed(optional) relative_status intimacy in_group
    def rs(r): return r.get('relative_status',r.get('rs'))
    def im(r): return r.get('intimacy',r.get('im'))
    def ig(r): return r.get('in_group',r.get('ig'))
    n=len(gs)
    if n==0: print(f"{name}: n=0"); return
    e=lambda f: sum(f(P[g])==f(Q[g]) for g in gs)
    s=f"{name}: n={n} rs_eq={e(rs)}/{n} ig_eq={e(ig)}/{n} im<=150={sum(abs(im(P[g])-im(Q[g]))<=150 for g in gs)}/{n}"
    s+=f" ig_kappa={kappa([ig(P[g]) for g in gs],[ig(Q[g]) for g in gs]):.2f} ig_tab(A.row,B.col)={tab([ig(P[g]) for g in gs],[ig(Q[g]) for g in gs])}"
    if all('addressed' in P[g] and 'addressed' in Q[g] for g in gs):
        s+=f" addr_eq={e(lambda r:r['addressed'])}/{n} addr_kappa={kappa([P[g]['addressed'] for g in gs],[Q[g]['addressed'] for g in gs]):.2f} addr_tab={tab([P[g]['addressed'] for g in gs],[Q[g]['addressed'] for g in gs])}"
    print(s)
L={"en-US":[g for g in G if G[g]['lang']=="en-US"],"ja-JP":[g for g in G if G[g]['lang']=="ja-JP"],"pooled":list(G)}
print("== (1) A vs B")
for k,gs in L.items(): cmp(k,Aa,Bb,gs)
oldg=[g for g in G if not G[g]['new']]; newg=[g for g in G if G[g]['new']]
print("== (2) blind check: A vs old latest, the 56")
for k,gs in L.items():
    gg=[g for g in gs if g in oldg]
    cmp(k+" all",Aa,old,gg)
    cmp(k+" A.addr=true",Aa,old,[g for g in gg if Aa[g]['addressed']])
    cmp(k+" A.addr=false",Aa,old,[g for g in gg if not Aa[g]['addressed']])
print("differing in_group (old->A):")
for g in oldg:
    if old[g]['ig']!=Aa[g]['in_group']: print(" ",g,old[g]['ig'],"->",Aa[g]['in_group'],"addr",Aa[g]['addressed'])
print("differing any label, old vs A:")
for g in oldg:
    o=old[g];a=Aa[g]
    if (o['rs'],o['im'],o['ig'])!=(a['relative_status'],a['intimacy'],a['in_group']): print("  %s old %s/%d/%s  A %s/%d/%s addr=%s"%(g,o['rs'],o['im'],o['ig'],a['relative_status'],a['intimacy'],a['in_group'],a['addressed']))
print("== (3) A vs vgrow @1, the 42")
for k,gs in L.items(): cmp(k,Aa,{g:lab[g][1] for g in newg},[g for g in gs if g in newg])
print("== (4) A addressed vs round-1 encoder addressee")
enc={}
RD=os.environ.get("ROUND1","runs/growth-base/round-1")+"/"
for p in glob.glob(RD+"*.prompt"):
    pt=open(p).read()
    if not pt.startswith("Encode the line"): continue
    ap=p[:-7]+".answer"
    if not os.path.exists(ap): continue
    # the line text; take the item whose text appears in the prompt
    hit=[g for g in G if G[g]['text'] in pt]
    if len(hit)!=1: continue
    try: a=json.load(open(ap))
    except Exception: continue
    enc[hit[0]]=a.get('discourse',{}).get('addressee') is not None
print("encoder answers matched:",len(enc),"of 98")
for k,gs in list(L.items()):
    gg=[g for g in gs if g in enc]
    print(k,"n=",len(gg),"A.row,enc.col",tab([Aa[g]['addressed'] for g in gg],[enc[g] for g in gg]),"kappa=%.2f"%kappa([Aa[g]['addressed'] for g in gg],[enc[g] for g in gg]))
for nm,grp in (("old",oldg),("new",newg)):
    gg=[g for g in grp if g in enc]; print(nm,"enc-addressee true:",sum(enc[g] for g in gg),"of",len(gg),"; A addressed true among old/new:",sum(Aa[g]['addressed'] for g in grp),"of",len(grp))
print("== (5) in_group true counts, old/new x addressed, before -> after")
for nm,grp in (("old56",oldg),("new42",newg)):
    for ad in (True,False):
        gg=[g for g in grp if Aa[g]['addressed']==ad]
        before=sum(old[g]['ig'] for g in gg); after=sum(Aa[g]['in_group'] for g in gg)
        print(f"{nm} A.addressed={ad}: n={len(gg)} in_group true before={before} after={after}")
print("totals before",sum(old[g]['ig'] for g in G),"after",sum(Aa[g]['in_group'] for g in G))
print("A addressed true:",sum(Aa[g]['addressed'] for g in G),"B:",sum(Bb[g]['addressed'] for g in G))
