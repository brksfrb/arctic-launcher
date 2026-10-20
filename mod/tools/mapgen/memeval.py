import sys,pickle,re
sys.path.insert(0,'.')
D=r'C:\Users\burak\AppData\Local\Temp'
c,mem=pickle.load(open(D+r'\mm132\match.pkl','rb'))
import names
cls,members=names.mojmap()   # mojmap->obf ; members: (obf class, rettype/fieldtype mojmap, name mojmap, params, obf name)
# tiny official->intermediary members
off={}   # (class off, name off, desc off) -> intermediary name
cur=None
for l in open(D+r'\mm132\i132.tiny',encoding='utf-8'):
    p=l.rstrip('\n').split('\t')
    if p[0]=='c': cur=p[1]
    elif len(p)>4 and p[0]=='' and p[1] in('f','m'): off[(cur,p[3],p[2])]=p[4]
yarn={}  # (intermediary class, intermediary name, desc?) -> yarn name
icls={}
for l in open(D+r'\mm132\i132.tiny',encoding='utf-8'):
    p=l.rstrip('\n').split('\t')
    if p[0]=='c': icls[p[1]]=p[2]
ycur=None
for l in open(D+r'\ly132\map.tiny',encoding='utf-8'):
    p=l.rstrip('\n').split('\t')
    if p[0]=='c': ycur=p[1]
    elif len(p)>4 and p[0]=='' and p[1] in('f','m'): yarn[(ycur,p[3])]=p[4]
want=sys.argv[1]
a=cls[want].replace('.','/')
# mojmap member names by obf
mm={}
for oc,ty,nm,params,obf in members:
    if oc==cls[want]: mm.setdefault(obf,[]).append((ty,nm,params))
for (ca,name,desc),(nb,db) in sorted(mem.items(), key=lambda kv:kv[0]):
    if ca!=a: continue
    isf=desc.startswith('F:')
    d=desc[2:] if isf else desc
    cb=c[ca]
    inter=off.get((cb,nb,db),'?')
    y=yarn.get((icls.get(cb,''),inter),'')
    print(('F ' if isf else 'M '), name, d[:50], '->', nb, inter, y, mm.get(name,'')[:1])
