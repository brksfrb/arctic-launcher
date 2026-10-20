import sys,pickle,re,glob,os
sys.path.insert(0,'.')
D=r'C:\Users\burak\AppData\Local\Temp'
c,mem=pickle.load(open(D+r'\mm132\match.pkl','rb'))
import names
cls,members=names.mojmap()   # mojmap->obf
root=r'C:\Users\burak\RustroverProjects\arctic-launcher\mod\versions\fabric\src\main\java'
used={}
for f in glob.glob(root+'/**/*.java',recursive=True):
    t=open(f,encoding='utf-8').read()
    # only the 1.14-active code: strip blocks gated for newer versions roughly: keep all
    for m in re.finditer(r'((?:net\.minecraft|com\.mojang)\.[a-z0-9_.]*[A-Z]\w*(?:\.[A-Z]\w*)*)',t):
        n=m.group(1)
        parts=n.split('.')
        for i in range(len(parts),0,-1):
            cand='.'.join(parts[:i-1])+('$'+parts[i-1] if False else '')
        # try progressively with $ for nested
        for i in range(len(parts),0,-1):
            nm='.'.join(parts[:i])
            if nm in cls: used.setdefault(nm,set()).add(os.path.basename(f)); break
            # nested
            for k in range(i-1,0,-1):
                nm2='.'.join(parts[:k])+'$'+'$'.join(parts[k:i])
                if nm2 in cls: used.setdefault(nm2,set()).add(os.path.basename(f)); break
            else: continue
            break
miss=[n for n in used if cls[n].replace('.','/') not in c]
print(len(used),'used classes in 1.14.4;',len(miss),'unmatched')
for n in sorted(miss): print(n, sorted(used[n])[:2])
