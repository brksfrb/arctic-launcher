"""Second-pass, reference-led shoulder pets with Minecraft-first proportions."""
from __future__ import annotations
import math
from build import Sculpture, box, diamond, feather, tapered, save, quat_y, quat_z

BLACK=(17,19,23); DARK=(28,31,36); SOFT=(43,47,55)
MID=(65,70,79); EDGE=(91,98,109); PALE=(211,219,221)
WHITE=(250,251,248); AQUA=(29,151,142); AQUA_HI=(79,216,195)
PINK=(226,105,119); TONGUE=(246,132,144)
FIRE=(255,101,25); AMBER=(255,174,35); LEMON=(255,235,100)
SNOW=(234,240,239); SHADOW=(178,190,196); FROST=(204,216,222)

def quat_x(angle):
    return [math.sin(angle/2),0.0,0.0,math.cos(angle/2)]

def smoothstep(start,end,value):
    u=max(0.0,min(1.0,(value-start)/(end-start)))
    return u*u*(3-2*u)


def blink_scale(t, start):
    """A quick close, tiny hold, and reopen; the lid is hinged at its top."""
    closed=smoothstep(start,start+.07,t)*(1-smoothstep(start+.14,start+.23,t))
    return [1.0,max(.001,closed),1.0]


def paint(mesh, center, size, face, pattern, colors):
    """Place deliberate pixel clusters on a model face, not random noise."""
    cx,cy,cz=center;w,h,d=size;n=len(pattern);m=max(map(len,pattern))
    for row,line in enumerate(pattern):
        for col,ch in enumerate(line):
            if ch=='.':continue
            tint=colors[ch]
            u0=col/m;u1=(col+1)/m;v0=row/n;v1=(row+1)/n
            if face=='front':
                z=cz-d/2-.025
                p=[(cx-w/2+u0*w,cy+h/2-v0*h,z),(cx-w/2+u1*w,cy+h/2-v0*h,z),
                   (cx-w/2+u1*w,cy+h/2-v1*h,z),(cx-w/2+u0*w,cy+h/2-v1*h,z)]
            elif face=='back':
                z=cz+d/2+.025
                p=[(cx+w/2-u0*w,cy+h/2-v0*h,z),(cx+w/2-u1*w,cy+h/2-v0*h,z),
                   (cx+w/2-u1*w,cy+h/2-v1*h,z),(cx+w/2-u0*w,cy+h/2-v1*h,z)]
            elif face in ('left','right'):
                x=cx+(-w/2-.025 if face=='left' else w/2+.025)
                p=[(x,cy+h/2-v0*h,cz-d/2+u0*d),(x,cy+h/2-v0*h,cz-d/2+u1*d),
                   (x,cy+h/2-v1*h,cz-d/2+u1*d),(x,cy+h/2-v1*h,cz-d/2+u0*d)]
            else:
                raise ValueError(face)
            mesh.tri(p[0],p[1],p[2],tint);mesh.tri(p[0],p[2],p[3],tint)


def shadow_pup():
    c=7.5
    body=Sculpture(); head=Sculpture(); tail=Sculpture(); jaw=Sculpture(); lids=Sculpture()
    # A low, broad animal body; the last version was tall like a tiny person.
    box(body,(c,26.15,.64),(4.85,4.25,4.78),DARK,BLACK,SOFT,MID)
    box(body,(c,27.91,-1.50),(3.76,.53,.41),SOFT,DARK,EDGE)
    for side in ('left','right'):
        paint(body,(c,26.15,.64),(4.85,4.25,4.78),side,
              ['..a..','..ab.','.abb.','..bb.','...b.'],
              {'a':SOFT,'b':MID})
    paint(body,(c,26.15,.64),(4.85,4.25,4.78),'back',
          ['..aa..','.aabb.','..bb..','..bb..','......'],{'a':SOFT,'b':MID})
    for x in (c-1.38,c+1.38):
        box(body,(x,24.55,-1.22),(1.66,1.66,2.42),SOFT,DARK,MID)
        box(body,(x,23.82,-2.13),(1.79,.55,1.40),MID,DARK,EDGE)
        for dx in (-.47,0,.47):
            box(body,(x+dx,23.74,-2.87),(.22,.19,.26),PALE,EDGE)
    # Wide cube-like head and low outward ears are closer to the reference.
    box(head,(c,30.62,-.94),(5.76,5.42,5.04),SOFT,BLACK,MID,EDGE)
    box(head,(c,33.21,-1.03),(4.82,.31,4.07),MID,DARK,EDGE)
    paint(head,(c,30.62,-.94),(5.76,5.42,5.04),'front',
          ['a....a','aa..aa','b....b','b....b','.c..c.','..cc..'],
          {'a':MID,'b':DARK,'c':SOFT})
    for side in ('left','right'):
        paint(head,(c,30.62,-.94),(5.76,5.42,5.04),side,
              ['bb....','bba...','.aa...','..aaa.','...cc.','....c.'],
              {'a':SOFT,'b':MID,'c':EDGE})
    paint(head,(c,30.62,-.94),(5.76,5.42,5.04),'back',
          ['..aa..','abbba.','bbbbbb','cbbbbc','.cccc.','..cc..'],
          {'a':MID,'b':SOFT,'c':DARK})
    for sign in (-1,1):
        x=c+sign*2.32
        outline=[(x-sign*.73,33.12,-1.75),(x-sign*.25,34.33,-1.65),
                 (x+sign*.80,34.39,-1.38),(x+sign*1.08,33.04,-1.18)]
        feather(head,outline,DARK,SOFT,edge=BLACK,thick=.76)
        box(head,(c+sign*1.26,31.35,-3.55),(1.34,1.08,.26),DARK,BLACK,SOFT)
        box(head,(c+sign*1.26,31.50,-3.72),(.60,.68,.18),WHITE)
        box(head,(c+sign*1.32,31.33,-3.84),(.22,.34,.12),BLACK)
        box(head,(c+sign*1.27,31.02,-3.70),(.88,.16,.13),AQUA)
        box(head,(c+sign*1.27,32.48,-3.59),(1.47,.26,.26),DARK)
        box(lids,(c+sign*1.26,31.48,-3.98),(1.47,1.10,.15),SOFT,DARK,MID)
        box(lids,(c+sign*1.26,30.96,-4.08),(1.17,.11,.11),BLACK)
    box(head,(c,29.45,-3.32),(3.52,1.67,1.37),MID,DARK,EDGE)
    box(head,(c,29.84,-4.15),(1.63,.78,.76),DARK,BLACK,MID)
    box(head,(c,29.72,-4.57),(1.31,.42,.16),BLACK)
    # Separate lower jaw: a tiny pink tongue peeks out of a dark mouth.
    box(jaw,(c,28.51,-3.74),(2.27,.46,.96),BLACK,DARK,SOFT)
    box(jaw,(c,28.23,-4.17),(.67,.22,.14),TONGUE,PINK)
    for x in (c-.74,c+.74):box(jaw,(x,28.60,-4.22),(.20,.20,.18),WHITE)
    # The reference has a proud, long tail, not a tight fox curl.
    tail.beam((c+2.17,26.80,2.12),(c+4.12,27.77,2.53),.77,.84,(DARK,SOFT,BLACK))
    tail.beam((c+4.12,27.77,2.53),(c+5.58,29.84,2.69),.84,.55,(SOFT,DARK,BLACK))
    tail.beam((c+5.58,29.84,2.69),(c+6.03,31.38,2.67),.55,.15,(MID,SOFT,DARK))
    for i in range(4):
        x=c+3.5+i*.63;y=28.0+i*.73
        box(tail,(x,y,3.05),(.68,.42,.31),BLACK,DARK)
    hp=[c,30.62,-.94]; jp=[c,28.44,-3.74];tp=[c+2.17,26.80,2.12]; ep=[c,32.03,-3.98]
    parts=[('pup_body',body,[0,0,0]),('pup_head',head,hp),('lower_jaw',jaw,jp),
           ('long_tail',tail,tp),('eyelids',lids,ep)]
    nodes=[{'name':'pup_body','mesh':0,'children':[1,3]},
           {'name':'pup_head','mesh':1,'translation':hp,'children':[2,4]},
           {'name':'lower_jaw','mesh':2,'translation':[jp[i]-hp[i] for i in range(3)]},
           {'name':'long_tail','mesh':3,'translation':tp},
           {'name':'eyelids','mesh':4,'translation':[ep[i]-hp[i] for i in range(3)]}]
    tracks=[(1,'rotation',lambda t:quat_y(.075*math.sin(2*math.pi*t/3.2))),
            (2,'rotation',lambda t:quat_z(.05*math.sin(2*math.pi*t/3.2))),
            (3,'rotation',lambda t:quat_z(.27*math.sin(8*math.pi*t/3.2))),
            (4,'scale',lambda t:blink_scale(t,1.40))]
    save('shadow_pup',parts,nodes,tracks)


def ember_hatchling():
    c=7.5
    body=Sculpture();head=Sculpture();left=Sculpture();right=Sculpture();tail=Sculpture();jaw=Sculpture();lids=Sculpture()
    # Horizontal, hovering dragon with a clear belly and hanging claws.
    box(body,(c,29.04,.87),(3.96,3.05,5.94),DARK,BLACK,SOFT,MID)
    for side in ('left','right'):
        paint(body,(c,29.04,.87),(3.96,3.05,5.94),side,
              ['..a...','.aba..','abbaa.','.bbb..','..cc..'],
              {'a':MID,'b':SOFT,'c':BLACK})
    paint(body,(c,29.04,.87),(3.96,3.05,5.94),'back',
          ['..aa..','.abba.','..bb..','..cc..','......'],
          {'a':MID,'b':SOFT,'c':BLACK})
    for y in (27.95,28.59):
        box(body,(c,y,-2.00),(2.63,.27,.29),MID,DARK,EDGE)
    for x in (c-1.37,c+1.37):
        box(body,(x,26.90,-1.34),(1.16,1.31,1.34),SOFT,DARK,MID)
        box(body,(x,26.27,-1.98),(1.27,.41,1.16),DARK,BLACK,SOFT)
        for dx in (-.36,.04,.36):
            box(body,(x+dx,26.10,-2.56),(.18,.22,.40),PALE,EDGE)
        box(body,(x,27.17,2.60),(1.27,1.17,1.38),DARK,BLACK,SOFT)
        box(body,(x,26.57,3.15),(1.27,.35,1.03),SOFT,DARK,MID)
    # Large square head, deep cheeks, long muzzle, open mouth, bright eyes.
    box(head,(c,31.04,-2.03),(5.66,4.65,4.46),SOFT,BLACK,MID,EDGE)
    paint(head,(c,31.04,-2.03),(5.66,4.65,4.46),'front',
          ['a....a','aa..aa','b....b','c....c','.cccc.'],
          {'a':MID,'b':DARK,'c':BLACK})
    for side in ('left','right'):
        paint(head,(c,31.04,-2.03),(5.66,4.65,4.46),side,
              ['aa....','aaa...','.bb...','..bbb.','...cc.'],
              {'a':MID,'b':SOFT,'c':BLACK})
    paint(head,(c,31.04,-2.03),(5.66,4.65,4.46),'back',
          ['..aa..','.abba.','bbbbbb','..cc..','......'],
          {'a':MID,'b':SOFT,'c':BLACK})
    for sign in (-1,1):
        x=c+sign*2.12
        head.beam((x,33.00,-.67),(x+sign*.54,34.50,.47),.57,.14,(DARK,MID,BLACK))
        head.beam((x+sign*.54,34.50,.47),(x+sign*.49,34.89,1.07),.14,.05,(SOFT,DARK))
        box(head,(c+sign*1.19,31.61,-4.39),(1.17,1.46,.24),WHITE,DARK,PALE)
        box(head,(c+sign*1.37,31.26,-4.56),(.29,.67,.10),BLACK)
        box(head,(c+sign*1.08,32.01,-4.57),(.22,.29,.09),WHITE)
        box(head,(c+sign*1.25,32.52,-4.37),(1.57,.27,.28),DARK)
        box(lids,(c+sign*1.19,31.61,-4.76),(1.33,1.54,.16),SOFT,DARK,MID)
        box(lids,(c+sign*1.19,30.88,-4.86),(1.10,.12,.12),BLACK)
    box(head,(c,29.56,-4.04),(3.97,1.32,2.26),MID,DARK,EDGE)
    for x in (c-1.18,c+1.18):
        box(head,(x,29.77,-5.23),(.40,.31,.18),BLACK)
    box(head,(c,30.16,-5.18),(3.69,.28,.24),SOFT,DARK,MID)
    box(head,(c,29.52,-5.24),(2.19,.46,.26),BLACK)
    box(jaw,(c,28.73,-4.59),(3.48,.71,1.43),BLACK,DARK,SOFT)
    box(jaw,(c,28.42,-5.38),(2.21,.25,.14),PALE)
    box(jaw,(c,29.11,-4.88),(2.46,.12,.75),(118,43,52),BLACK)
    for x in (c-.63,c+.63):box(head,(x,29.06,-5.41),(.32,.45,.26),WHITE)
    # Folded bat-like wings are volumetric, with visible finger spines.
    for sign,m in ((-1,left),(1,right)):
        x=c+sign*1.92
        outline=[(x,29.53,1.43),(x+sign*1.28,31.14,1.92),
                 (x+sign*3.63,32.08,2.42),(x+sign*3.23,29.81,2.55),
                 (x+sign*2.53,29.39,2.48),(x+sign*1.99,28.12,2.29)]
        feather(m,outline,DARK,BLACK,edge=MID,thick=.76)
        m.beam(outline[0],outline[2],.26,.10,(SOFT,MID,BLACK))
        m.beam(outline[0],outline[3],.18,.07,(SOFT,MID,BLACK))
        for p in outline[2:5]:box(m,(p[0],p[1],p[2]-.06),(.23,.27,.29),BLACK)
    tail.beam((c,27.40,3.18),(c+.44,26.43,4.65),.68,.57,(DARK,SOFT,BLACK))
    tail.beam((c+.44,26.43,4.65),(c+1.11,26.16,6.32),.57,.37,(SOFT,DARK,BLACK))
    tail.beam((c+1.11,26.16,6.32),(c+1.37,25.18,7.22),.37,.07,(MID,DARK,BLACK))
    for z,y in ((2.86,30.53),(3.48,30.19),(4.20,29.63)):
        feather(body,[(c-.53,y-.45,z),(c,y+.55,z+.10),(c+.53,y-.45,z)],
                MID,SOFT,edge=BLACK,thick=.60)
    hp=[c,31.04,-2.03];lp=[c-1.92,29.53,1.43];rp=[c+1.92,29.53,1.43]
    tp=[c,27.40,3.18]
    jp=[c,29.24,-3.82]
    parts=[('dragon_body',body,[0,0,0]),('dragon_head',head,hp),
           ('left_wing',left,lp),('right_wing',right,rp),
           ('tail',tail,tp),('lower_jaw',jaw,jp)]
    nodes=[{'name':'dragon_body','mesh':0,'children':[1,2,3,4]},
           {'name':'dragon_head','mesh':1,'translation':hp,'children':[5]},
           {'name':'left_wing','mesh':2,'translation':lp},
           {'name':'right_wing','mesh':3,'translation':rp},
           {'name':'tail','mesh':4,'translation':tp},
           {'name':'lower_jaw','mesh':5,'translation':[jp[i]-hp[i] for i in range(3)]}]
    def jaw_angle(t):
        return -.68*(smoothstep(.94,1.28,t)-smoothstep(2.45,2.82,t))
    tracks=[(1,'rotation',lambda t:quat_y(.047*math.sin(2*math.pi*t/4.8))),
            (2,'rotation',lambda t:quat_z(.23*math.sin(6*math.pi*t/4.8))),
            (3,'rotation',lambda t:quat_z(-.23*math.sin(6*math.pi*t/4.8))),
            (4,'rotation',lambda t:quat_y(.12*math.sin(2*math.pi*t/4.8))),
            (5,'rotation',lambda t:quat_x(jaw_angle(t)))]
    # One short breath per 4.8-second loop. Each flame tongue is launched
    # from the mouth, travels forward and upward, then vanishes before reset.
    # Emerge from the mouth opening below the upper teeth, not the snout.
    origin=[c,28.35,-5.55]
    for i in range(5):
        tongue=Sculpture()
        width=.24+.06*(i%3)
        height=.38+.10*(i%4)
        tongue.beam((c,28.35,-5.55),(c+.10,28.35+height,-5.68),
                    width,.025,((FIRE,AMBER,LEMON) if i%3 else (AMBER,FIRE,LEMON)))
        tongue.beam((c+.01,28.42,-5.61),(c+.04,28.34+height*.69,-5.70),
                    width*.58,.018,(LEMON,WHITE,AMBER))
        if i%2==0:
            diamond(tongue,c+.17,28.50,-5.76,.11,WHITE,LEMON)
        index=len(parts)
        parts.append((f'fire_tongue_{i}',tongue,origin))
        nodes.append({'name':f'fire_tongue_{i}','mesh':index,'translation':origin})
        nodes[0]['children'].append(index)
        launch=1.28+i*.11
        lateral=.42+.18*(i%4)
        rise=.30+.16*(i%3)
        def age(t,launch=launch):return t-launch
        def position(t,age=age,lateral=lateral,rise=rise):
            a=age(t)
            p=a/.68 if 0<a<.68 else 0
            return [origin[0]+lateral*p,origin[1]+rise*p,
                    origin[2]-4.10*p]
        def visibility(t,age=age):
            a=age(t)
            s=min(1,a/.08,(.68-a)/.14) if 0<a<.68 else 0
            return [max(.001,s)]*3
        tracks.append((index,'translation',position))
        tracks.append((index,'scale',visibility))
    ep=[c,32.38,-4.76]
    index=len(parts)
    parts.append(('eyelids',lids,ep))
    nodes.append({'name':'eyelids','mesh':index,
                  'translation':[ep[i]-hp[i] for i in range(3)]})
    nodes[1]['children'].append(index)
    tracks.append((index,'scale',lambda t:blink_scale(t,3.30)))
    save('ember_hatchling',parts,nodes,tracks,period=4.8,keys=120)


def cloud_wisp():
    c=-7.5
    body=Sculpture();legs=Sculpture();lids=Sculpture()
    # Restrained little ghast-like friend: a simple square face and two legs.
    box(body,(c,29.40,.05),(5.46,5.08,4.66),SNOW,SHADOW,WHITE,FROST)
    box(body,(c,31.82,-.12),(4.99,.26,4.12),WHITE,FROST)
    for sign in (-1,1):
        x=c+sign*1.15
        box(body,(x,29.81,-2.43),(.72,.68,.17),BLACK)
        box(body,(x,29.93,-2.54),(.20,.19,.08),WHITE)
        box(body,(x,28.83,-2.46),(.92,.33,.15),PINK)
        box(body,(x,31.08,-2.43),(.38,.38,.13),SHADOW)
        box(lids,(x,29.81,-2.67),(.86,.82,.14),SNOW,SHADOW,WHITE)
        box(lids,(x,29.43,-2.76),(.68,.10,.10),SHADOW)
        box(legs,(x,25.69,.11),(1.07,2.37,1.54),FROST,SHADOW,SNOW)
        box(legs,(x,24.54,-.12),(1.05,.39,1.21),SHADOW,FROST,WHITE)
    box(body,(c,28.79,-2.51),(.71,.45,.17),BLACK)
    bp=[c,29.40,.05];lp=[c,25.69,.11];ep=[c,30.22,-2.67]
    parts=[('wisp_body',body,[0,0,0]),('legs',legs,lp),('eyelids',lids,ep)]
    nodes=[{'name':'wisp_body','mesh':0,'children':[1,2]},
           {'name':'legs','mesh':1,'translation':lp},
           {'name':'eyelids','mesh':2,'translation':ep}]
    tracks=[(0,'translation',lambda t:[0,.43*math.sin(2*math.pi*t/3.2),0]),
            (1,'rotation',lambda t:quat_z(.04*math.sin(2*math.pi*t/3.2))),
            (2,'scale',lambda t:blink_scale(t,1.90))]
    save('cloud_wisp',parts,nodes,tracks)


if __name__=='__main__':
    shadow_pup();ember_hatchling();cloud_wisp()
