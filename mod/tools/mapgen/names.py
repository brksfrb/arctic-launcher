"""Names for both sides: mojmap for 1.14.4, intermediary and yarn for 1.13.2."""
import re
D=r'C:\Users\burak\AppData\Local\Temp'
def mojmap():
    """class obf->mojmap, and (obf class, obf member name, obf desc)->mojmap name."""
    cls={}; order=[]
    cur=None
    members=[]
    for l in open(D+r'\mm1144\map.txt',encoding='utf-8'):
        if l.startswith('#'): continue
        if not l.startswith(' '):
            m=re.match(r'(\S+) -> (\S+):',l); cur=(m.group(1),m.group(2)); cls[cur[0]]=cur[1]; continue
        l=l.strip()
        m=re.match(r'(?:\d+:\d+:)?(.+?) (\S+?)(\((.*)\))?(?::\d+:\d+)? -> (\S+)$',l)
        if not m: continue
        members.append((cur[0],m.group(1),m.group(2),m.group(4),m.group(5)))
    return cls,members
def tiny(path, a_col=1):
    """tiny v2: returns class a->b, field (class a,name a,desc a)->name b, methods likewise."""
    c={};f={};mm={};cur=None
    for l in open(path,encoding='utf-8'):
        p=l.rstrip('\n').split('\t')
        if p[0]=='c': cur=p[1]; c[p[1]]=p[2]
        elif len(p)>3 and p[0]=='' and p[1]=='f': f[(cur,p[3],p[2])]=p[3+1-1] if False else p[3]
    return c
