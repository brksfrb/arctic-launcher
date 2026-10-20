import sys,time,pickle
sys.path.insert(0,'.')
import classfile,match
D=r'C:\Users\burak\AppData\Local\Temp\mm132'
t=time.time()
a=classfile.load(D+r'\c144.jar'); b=classfile.load(D+r'\c132.jar')
m=match.Matcher(a,b)
c,mem=m.run()
print(len(c),len(mem),time.time()-t)
pickle.dump((c,mem),open(D+r'\match.pkl','wb'))
