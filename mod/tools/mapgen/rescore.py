import sys,pickle,time
sys.path.insert(0,'.')
import classfile,match
D=r'C:\Users\burak\AppData\Local\Temp\mm132'
a=classfile.load(D+r'\c144.jar'); b=classfile.load(D+r'\c132.jar')
c,mem=pickle.load(open(D+r'\match.pkl','rb'))
m=match.Matcher(a,b)
m.cmap=dict(c); m.cmap_rev={v:k for k,v in c.items()}
t=time.time()
cands=m.candidates()
print(time.time()-t)
ca='com/mojang/blaze3d/platform/GlStateManager'
print(cands.get(ca))
