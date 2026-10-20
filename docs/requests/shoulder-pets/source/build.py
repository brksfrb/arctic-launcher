"""Build three original Minecraft-scale shoulder companions as animated GLBs.

Run from any directory. Geometry is authored in player pixel units; the
borrowed Arctic GLB writer only serializes this original geometry.
"""
from __future__ import annotations

import math
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[4]
sys.path.insert(0, str(ROOT / "crates/arctic-cosmetics/assets/tools"))

from make_codex_meshes import animation_of
from make_dragon_wings import Sculpture
from make_mesh_pilots import Glb, primitive, quat_y, quat_z
from make_snow_owl import tapered, feather

OUT = Path(__file__).parent

INK=(18,22,28); COAL=(35,39,48); SLATE=(59,64,75); CHAR=(83,88,96)
SILVER=(143,151,156); CREAM=(249,242,220); WHITE=(255,255,246)
TEAL=(30,205,187); ICE=(127,235,224); PINK=(246,103,133)
AMBER=(255,169,51); GOLD=(255,217,93); HOT=(255,94,28)
LILAC=(172,193,246); FROST=(208,234,250); BLUE=(92,156,202)


def box(m, c, s, front, side=None, top=None, back=None):
    x,y,z=c; w,h,d=(q/2 for q in s)
    side=side or front; top=top or front; back=back or side
    p=[(x-w,y-h,z-d),(x+w,y-h,z-d),(x+w,y+h,z-d),(x-w,y+h,z-d),
       (x-w,y-h,z+d),(x+w,y-h,z+d),(x+w,y+h,z+d),(x-w,y+h,z+d)]
    for face,col in zip(((0,1,2,3),(4,7,6,5),(0,4,5,1),(3,2,6,7),
                         (0,3,7,4),(1,5,6,2)),
                        (front,back,side,top,side,side)):
        a,b,c,d=(p[i] for i in face)
        m.tri(a,b,c,col);m.tri(a,c,d,col)


def diamond(m,x,y,z,r,col,edge):
    p=[(x,y+r,z),(x+r,y,z),(x,y-r,z),(x-r,y,z)]
    a=(x,y,z-r*.52);b=(x,y,z+r*.52)
    for i in range(4):
        j=(i+1)%4
        m.tri(a,p[i],p[j],col);m.tri(b,p[j],p[i],edge)


def mesh(glb,m,pivot,name):
    p=primitive(glb,m.gltf(),pivot,0)
    p['attributes']['NORMAL']=glb.add(m.gltf()['normal'],'VEC3',5126)
    return {'name':name,'primitives':[p]}


def save(name,parts,nodes,tracks,period=3.2,keys=80):
    glb=Glb()
    meshes=[mesh(glb,m,p,n) for n,m,p in parts]
    has_fire=any(n.startswith('fire_tongue_') for n,_,_ in parts)
    if has_fire:
        for part, (part_name,_,_) in zip(meshes,parts):
            if part_name.startswith('fire_tongue_'):
                part['primitives'][0]['material']=1
    materials=[{'name':'voxel_paint','pbrMetallicRoughness':{
        'baseColorFactor':[1,1,1,1],'metallicFactor':0,
        'roughnessFactor':.86},'doubleSided':True}]
    if has_fire:
        materials.append({'name':'ember_light','pbrMetallicRoughness':{
            'baseColorFactor':[1,1,1,1],'metallicFactor':0,
            'roughnessFactor':1},'emissiveFactor':[.8,.36,.04],
            'doubleSided':True})
    doc={'asset':{'version':'2.0','generator':'Arctic original shoulder pet trio'},
         'scene':0,'scenes':[{'nodes':[0]}], 'nodes':nodes,'meshes':meshes,
         'materials':materials,
         'animations':[animation_of(glb,period,keys,tracks)]}
    glb.write(doc,str(OUT/f'{name}.glb'))


def pup():
    cx=7.5
    body=Sculpture(); head=Sculpture(); tail=Sculpture(); ear_l=Sculpture(); ear_r=Sculpture()
    # An actual seat on the shoulder, without the raised gold platform.
    tapered(body,[(cx,23.75,.05,2.46,2.35,.43),
                  (cx,24.19,.05,2.52,2.36,.43)],front=INK,side=COAL,back=COAL,bevel=SLATE)
    tapered(body,[(cx,24.10,.15,1.95,1.68,.50),
                  (cx,25.05,.12,2.33,2.17,.61),
                  (cx,28.80,.18,2.37,2.15,.58),
                  (cx,29.55,.17,1.75,1.72,.45)],
            front=COAL,side=INK,back=SLATE,bevel=CHAR)
    # Chest bib and haunches make the animal read as seated from the side.
    tapered(body,[(cx,24.90,-1.93,1.25,.26,.18),
                  (cx,28.75,-2.06,1.42,.32,.20)],
            front=CHAR,side=SLATE,back=COAL,bevel=SILVER)
    for x in (cx-1.31,cx+1.31):
        box(body,(x,25.00,-1.45),(1.25,1.47,1.41),COAL,INK,SLATE)
        box(body,(x,24.42,-2.26),(1.18,.48,1.23),SLATE,INK,CHAR)
        for dx in (-.30,.05,.36):
            box(body,(x+dx,24.26,-2.88),(.16,.13,.24),SILVER)
    # Faceted head, protruding muzzle, two cheeks, tongue and bright eyes.
    tapered(head,[(cx,29.18,-.04,2.00,1.76,.44),
                  (cx,29.75,-.06,2.60,2.27,.65),
                  (cx,32.55,-.04,2.68,2.22,.63),
                  (cx,33.12,-.01,2.14,1.77,.49)],
            front=COAL,side=INK,back=SLATE,bevel=CHAR)
    for x in (cx-1.54,cx+1.54):
        box(head,(x,30.61,-2.33),(1.13,1.40,.79),SLATE,COAL,CHAR)
    tapered(head,[(cx,29.44,-2.11,.92,.61,.22),
                  (cx,30.55,-2.72,1.22,.75,.27),
                  (cx,31.01,-3.18,1.06,.55,.22)],
            front=COAL,side=INK,back=SLATE,bevel=CHAR)
    box(head,(cx,30.52,-3.40),(1.30,.69,.59),INK,COAL,SLATE)
    box(head,(cx,29.82,-3.36),(.65,.48,.22),PINK,HOT,PINK)
    for x in (cx-1.13,cx+1.13):
        box(head,(x,31.70,-2.52),(.74,.89,.19),TEAL,INK,ICE)
        box(head,(x-.15,31.94,-2.67),(.22,.28,.09),WHITE)
        box(head,(x,32.50,-2.51),(1.02,.24,.21),INK)
    # A small ice-blue collar remains visible in front and profile.
    box(body,(cx,29.26,-1.47),(4.55,.40,1.35),TEAL,INK,ICE)
    diamond(body,cx,29.12,-2.23,.37,ICE,TEAL)
    for sign,ear in ((-1,ear_l),(1,ear_r)):
        x=cx+sign*1.96
        tapered(ear,[(x,32.57,-.23,.63,.60,.16),
                     (x+sign*.26,34.62,-.28,.63,.49,.15),
                     (x+sign*.38,35.22,-.19,.13,.13,.08)],
                front=COAL,side=INK,back=SLATE,bevel=CHAR)
        box(ear,(x+sign*.15,33.88,-.88),(.50,1.38,.10),PINK)
    # Three substantial tapering sections, attached behind the body.
    tail.beam((cx+1.85,25.31,1.76),(cx+3.18,26.02,2.74),.71,.96,(COAL,SLATE,INK))
    tail.beam((cx+3.18,26.02,2.74),(cx+4.15,28.23,3.04),.96,.74,(SLATE,COAL,INK))
    tail.beam((cx+4.15,28.23,3.04),(cx+3.71,30.22,2.76),.74,.12,(SLATE,CHAR,INK))
    pivot=[cx,30.5,0]
    parts=[('body',body,[0,0,0]),('pup_head',head,pivot),
           ('left_ear',ear_l,[cx-1.96,33.2,-.23]),
           ('right_ear',ear_r,[cx+1.96,33.2,-.23]),
           ('wagging_tail',tail,[cx+1.85,25.31,1.76])]
    nodes=[{'name':'body','mesh':0,'children':[1,4]},
           {'name':'pup_head','mesh':1,'translation':pivot,'children':[2,3]},
           {'name':'left_ear','mesh':2,'translation':[-1.96,2.7,-.23]},
           {'name':'right_ear','mesh':3,'translation':[1.96,2.7,-.23]},
           {'name':'wagging_tail','mesh':4,'translation':[cx+1.85,25.31,1.76]}]
    tracks=[(1,'rotation',lambda t:quat_y(.075*math.sin(2*math.pi*t/3.2))),
            (2,'rotation',lambda t:quat_z(.055*math.sin(2*math.pi*t/3.2))),
            (3,'rotation',lambda t:quat_z(-.055*math.sin(2*math.pi*t/3.2))),
            (4,'rotation',lambda t:quat_y(.43*math.sin(8*math.pi*t/3.2)))]
    save('shadow_pup',parts,nodes,tracks)


def dragon():
    cx=7.55
    body=Sculpture();head=Sculpture();wing_l=Sculpture();wing_r=Sculpture();tail=Sculpture();flame=Sculpture()
    tapered(body,[(cx,23.90,.12,2.10,1.95,.41),(cx,24.25,.12,2.26,2.07,.43)],
            front=INK,side=COAL,back=SLATE,bevel=CHAR)
    tapered(body,[(cx,24.15,.20,1.65,1.72,.42),(cx,25.17,.20,2.08,2.24,.56),
                  (cx,28.81,.18,2.01,2.11,.55),(cx,29.52,.16,1.54,1.54,.43)],
            front=COAL,side=INK,back=SLATE,bevel=CHAR)
    tapered(body,[(cx,24.78,-1.93,1.16,.23,.18),
                  (cx,28.13,-2.04,1.16,.28,.20)],
            front=(119,76,54),side=COAL,back=SLATE,bevel=(180,91,50))
    for x in (cx-1.37,cx+1.37):
        box(body,(x,24.72,-1.64),(1.13,1.02,1.40),COAL,INK,CHAR)
        for dx in (-.29,.08,.33):
            box(body,(x+dx,24.32,-2.49),(.18,.27,.50),SILVER,COAL)
    for y in (26.2,27.65,29.0):
        diamond(body,cx,y,2.50,.58,SLATE,CHAR)
    tapered(head,[(cx,29.03,-.11,1.64,1.61,.42),
                  (cx,29.72,-.16,2.31,2.14,.57),
                  (cx,32.30,-.15,2.41,2.19,.57),
                  (cx,33.04,-.12,1.79,1.65,.42)],
            front=COAL,side=INK,back=SLATE,bevel=CHAR)
    tapered(head,[(cx,29.50,-2.02,1.07,.61,.22),
                  (cx,30.47,-3.01,1.43,.70,.27),
                  (cx,30.97,-3.33,1.31,.49,.22)],
            front=COAL,side=INK,back=SLATE,bevel=CHAR)
    box(head,(cx,30.50,-3.75),(.69,.30,.21),AMBER)
    for x in (cx-1.08,cx+1.08):
        box(head,(x,31.68,-2.45),(.65,.79,.20),AMBER,HOT,GOLD)
        box(head,(x-.13,31.93,-2.58),(.17,.18,.08),WHITE)
        box(head,(x,32.42,-2.45),(.91,.26,.20),INK)
    for x in (cx-1.68,cx+1.68):
        head.beam((x,32.51,.70),(x+(.49 if x>cx else -.49),34.39,.34),.49,.07,(COAL,SLATE,CHAR))
    for sign,wing in ((-1,wing_l),(1,wing_r)):
        x=cx+sign*1.84
        # Swept, closed membrane with finger bones: small enough for a pet.
        outline=[(x,28.89,1.53),(x+sign*1.45,29.98,2.00),
                 (x+sign*3.24,31.61,2.51),(x+sign*3.04,29.60,2.50),
                 (x+sign*2.18,28.75,2.43),(x+sign*1.90,27.53,2.29)]
        feather(wing,outline,(91,50,46),(54,42,52),edge=INK,thick=.48)
        wing.beam(outline[0],outline[2],.22,.08,(CHAR,SLATE))
        wing.beam(outline[0],outline[3],.16,.07,(CHAR,SLATE))
        diamond(wing,outline[2][0],outline[2][1],2.23,.29,AMBER,HOT)
    tail.beam((cx,25.33,1.69),(cx+.55,24.52,3.13),.61,.50,(COAL,INK,SLATE))
    tail.beam((cx+.55,24.52,3.13),(cx+1.10,25.29,4.55),.50,.34,(COAL,SLATE,INK))
    diamond(tail,cx+1.18,25.44,4.61,.65,HOT,AMBER)
    # Tiny flame glyph, fused to the pet asset rather than a particle system.
    for x,y,z,r,c in ((cx,31.10,-4.42,.57,AMBER),
                      (cx+.32,31.54,-4.45,.31,GOLD),
                      (cx-.42,31.01,-4.50,.25,HOT)):
        diamond(flame,x,y,z,r,c,HOT)
    hp=[cx,30.5,-.1];wp_l=[cx-1.84,28.89,1.53];wp_r=[cx+1.84,28.89,1.53]
    tp=[cx,25.33,1.69];fp=[cx,31.10,-4.42]
    parts=[('body',body,[0,0,0]),('dragon_head',head,hp),
           ('left_wing',wing_l,wp_l),('right_wing',wing_r,wp_r),
           ('forked_tail',tail,tp),('ember_breath',flame,fp)]
    nodes=[{'name':'body','mesh':0,'children':[1,2,3,4,5]},
           {'name':'dragon_head','mesh':1,'translation':hp},
           {'name':'left_wing','mesh':2,'translation':wp_l},
           {'name':'right_wing','mesh':3,'translation':wp_r},
           {'name':'forked_tail','mesh':4,'translation':tp},
           {'name':'ember_breath','mesh':5,'translation':fp}]
    tracks=[(1,'rotation',lambda t:quat_y(.07*math.sin(2*math.pi*t/3.2))),
            (2,'rotation',lambda t:quat_z(.27*math.sin(4*math.pi*t/3.2))),
            (3,'rotation',lambda t:quat_z(-.27*math.sin(4*math.pi*t/3.2))),
            (4,'rotation',lambda t:quat_y(.10*math.sin(2*math.pi*t/3.2))),
            (5,'scale',lambda t:[.67+.30*max(0,math.sin(2*math.pi*t/3.2)),
                                  .67+.30*max(0,math.sin(2*math.pi*t/3.2)),1])]
    save('ember_hatchling',parts,nodes,tracks)


def wisp():
    cx=-7.50
    core=Sculpture(); face=Sculpture(); tendrils=[]
    # Chamfered block body and true volume on all four sides.
    tapered(core,[(cx,25.62,.08,1.67,1.53,.41),
                  (cx,26.13,.08,2.50,2.25,.51),
                  (cx,30.28,.08,2.52,2.26,.52),
                  (cx,30.88,.08,1.86,1.73,.40)],
            front=WHITE,side=FROST,back=(185,218,239),bevel=CREAM)
    for x in (cx-1.60,cx+1.60):
        diamond(core,x,29.86,-2.29,.33,ICE,BLUE)
    # Off-centre curled crown and cool underside distinguish it from a ghast.
    core.beam((cx+.64,30.51,.41),(cx+1.10,31.45,.20),.41,.18,(WHITE,FROST))
    diamond(core,cx+1.14,31.54,.18,.38,LILAC,FROST)
    box(core,(cx,25.83,-.01),(3.56,.27,3.15),BLUE,FROST,ICE)
    for x in (cx-1.02,cx+1.02):
        box(face,(x,28.63,-2.30),(.75,1.16,.15),INK,COAL,SLATE)
        box(face,(x-.16,29.02,-2.41),(.20,.26,.08),WHITE)
        box(face,(x,27.40,-2.31),(.81,.33,.13),PINK)
    box(face,(cx,27.61,-2.33),(.49,.31,.14),COAL)
    for i,(dx,dz) in enumerate(((-1.33,-1.22),(-.45,-1.25),(.45,-1.25),(1.31,-1.19),
                                (-1.26,1.15),(-.03,1.21),(1.23,1.16))):
        m=Sculpture();x=cx+dx
        tapered(m,[(x,24.05+(.3 if i%2 else 0),dz,.21,.20,.10),
                   (x,25.83,dz,.38,.40,.17)],
                front=FROST,side=BLUE,back=WHITE,bevel=ICE)
        tendrils.append((m,[x,25.68,dz]))
    parts=[('body',core,[0,0,0]),('face',face,[cx,28.3,-2.3])]
    nodes=[{'name':'body','mesh':0,'children':list(range(1,9))},
           {'name':'face','mesh':1,'translation':[cx,28.3,-2.3]}]
    for i,(m,p) in enumerate(tendrils):
        parts.append((f'tendril_{i}',m,p))
        nodes.append({'name':f'tendril_{i}','mesh':i+2,'translation':p})
    tracks=[(0,'translation',lambda t:[0,.48*math.sin(2*math.pi*t/3.2),0])]
    tracks += [(i+2,'rotation',lambda t,i=i:quat_z(.075*math.sin(2*math.pi*t/3.2+i*.72)))
               for i in range(len(tendrils))]
    save('cloud_wisp',parts,nodes,tracks)


if __name__=='__main__':
    pup();dragon();wisp()
