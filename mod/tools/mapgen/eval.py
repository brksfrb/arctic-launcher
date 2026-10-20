import sys,pickle,re
sys.path.insert(0,'.')
D=r'C:\Users\burak\AppData\Local\Temp'
c,mem=pickle.load(open(D+r'\mm132\match.pkl','rb'))
import names
cls,members=names.mojmap()
obf2moj={v:k for k,v in cls.items()}
off2int={}
for l in open(D+r'\mm132\i132.tiny',encoding='utf-8'):
    p=l.rstrip('\n').split('\t')
    if p[0]=='c': off2int[p[1]]=p[2]
int2yarn={}
for l in open(D+r'\ly132\map.tiny',encoding='utf-8'):
    p=l.rstrip('\n').split('\t')
    if p[0]=='c': int2yarn[p[1]]=p[2]
def words(n):
    n=n.split('.')[-1].split('/')[-1]
    return set(w.lower() for w in re.findall(r'[A-Z][a-z0-9]*|[a-z0-9]+',n))
ok=bad=0; bads=[]
for a,b in c.items():
    moj=obf2moj.get(a); y=int2yarn.get(off2int.get(b,''))
    if not moj or not y or 'class_' in y: continue
    wa=words(moj); wb=words(y)
    if wa&wb: ok+=1
    else: bad+=1; bads.append((moj,y))
print(ok,bad)
for x in bads[:80]: print(x)
