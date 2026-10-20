"""Pixel-textured art pass over the shoulder-pet study geometry.

The texture is deliberately authored in small clusters at consistent texel
density and embedded in the GLB. These are the listed replacement models.
"""
from __future__ import annotations
import io
from pathlib import Path
import numpy as np
from PIL import Image

import build_v2 as v2
from build import Glb, primitive
from make_codex_meshes import animation_of

HERE=Path(__file__).parent
SIZE=512
atlas=Image.new('RGBA',(SIZE,SIZE),(255,255,255,255))
pixels=atlas.load()
cursor_x=2;cursor_y=2;row_height=0
style='pup'
white_uv=(.5/SIZE,1-.5/SIZE)


class ArtMesh(v2.Sculpture):
    def __init__(self):
        super().__init__();self.uvs=[]
    def tri(self,a,b,c,color,uvs=None,outward=None):
        super().tri(a,b,c,color,outward=outward)
        self.uvs.extend(uvs or (white_uv,white_uv,white_uv))
    def gltf(self):
        result=super().gltf()
        result['uv']=np.asarray(self.uvs,dtype=np.float32)
        return result


def clamp(n):return max(0,min(255,round(n)))
def shade(color,delta,blue=0):
    return tuple(clamp(v+delta+(blue if i==2 else 0)) for i,v in enumerate(color))+(255,)


def tile(w,h,base,kind):
    global cursor_x,cursor_y,row_height
    w=max(2,min(16,round(w)));h=max(2,min(16,round(h)))
    if cursor_x+w+3>=SIZE:
        cursor_x=2;cursor_y+=row_height+2;row_height=0
    if cursor_y+h+3>=SIZE:raise RuntimeError('Texture atlas full')
    x0,y0=cursor_x,cursor_y
    cursor_x+=w+2;row_height=max(row_height,h)
    for y in range(h):
        for x in range(w):
            # Intentional clusters: highlight at upper/outer faces, dark seam
            # below. These form actual model texture, not a noisy overlay.
            value=8 if kind=='top' else 4 if kind=='front' else -4 if kind=='back' else -2
            if y==0:value+=8
            if y>=h-2:value-=9
            if x==0:value+=4
            if x==w-1:value-=5
            if style=='pup':
                if kind in ('side','back'):
                    if (y in (1,2) and x in (1,2,3)) or (y in (3,4) and x in (2,3)):
                        value+=9
                    if y>=h-2 and x%3==1:value-=8
                if kind=='front' and y<=1 and 1<=x<w-1:value+=9
            elif style=='dragon':
                if kind in ('side','back'):
                    for sx,sy in ((w//3,2),(2*w//3,3),(w//2,h-2)):
                        if abs(x-sx)+abs(y-sy)<=1:value+=12
                    if y>=h-2:value-=5
                if kind=='top' and x in (w//3,2*w//3):value+=11
            else:
                if kind=='front' and x in (0,w-1):value-=3
                if kind=='side' and y>=h//2:value-=6
            pixels[x0+x,y0+y]=shade(base,value,2 if style=='dragon' else 0)
    # Extend edge colors into the padding to prevent texture bleeding.
    for x in range(w):
        pixels[x0+x,y0-1]=pixels[x0+x,y0]
        pixels[x0+x,y0+h]=pixels[x0+x,y0+h-1]
    for y in range(-1,h+1):
        pixels[x0-1,y0+y]=pixels[x0,y0+min(max(y,0),h-1)]
        pixels[x0+w,y0+y]=pixels[x0+w-1,y0+min(max(y,0),h-1)]
    # UVs return the exact texel rectangle; nearest sampling preserves pixels.
    return ((x0+.03)/SIZE,1-(y0+.03)/SIZE,(x0+w-.03)/SIZE,1-(y0+h-.03)/SIZE)


def painted_box(mesh,center,size,front,side=None,top=None,back=None):
    x,y,z=center;w,h,d=(q/2 for q in size)
    side=side or front;top=top or front;back=back or side
    corners=[(x-w,y-h,z-d),(x+w,y-h,z-d),(x+w,y+h,z-d),(x-w,y+h,z-d),
             (x-w,y-h,z+d),(x+w,y-h,z+d),(x+w,y+h,z+d),(x-w,y+h,z+d)]
    faces=((0,1,2,3),(4,7,6,5),(0,4,5,1),(3,2,6,7),(0,3,7,4),(1,5,6,2))
    face_specs=((size[0],size[1],front,'front'),(size[0],size[1],back,'back'),
                (size[2],size[1],side,'side'),(size[0],size[2],top,'top'),
                (size[2],size[1],side,'side'),(size[2],size[1],side,'side'))
    for f,(tw,th,col,kind) in zip(faces,face_specs):
        u0,v0,u1,v1=tile(tw,th,col,kind)
        p=[corners[i] for i in f]
        uv=((u0,v1),(u1,v1),(u1,v0),(u0,v0))
        mesh.tri(p[0],p[1],p[2],v2.WHITE,(uv[0],uv[1],uv[2]))
        mesh.tri(p[0],p[2],p[3],v2.WHITE,(uv[0],uv[2],uv[3]))


old_box=v2.box
def box(mesh,center,size,front,side=None,top=None,back=None):
    if min(size)>2.5 and max(size)>3.7:
        painted_box(mesh,center,size,front,side,top,back)
    else:
        old_box(mesh,center,size,front,side,top,back)


def save(name,parts,nodes,tracks,period=3.2,keys=80):
    glb=Glb();meshes=[]
    for part_name,m,pivot in parts:
        data=m.gltf();prim=primitive(glb,data,pivot,0)
        prim['attributes']['NORMAL']=glb.add(data['normal'],'VEC3',5126)
        meshes.append({'name':part_name,'primitives':[prim]})
    binary=io.BytesIO();atlas.save(binary,format='PNG',optimize=True)
    doc={'asset':{'version':'2.0','generator':'Arctic shoulder pet texture study'},
         'scene':0,'scenes':[{'nodes':[0]}],'nodes':nodes,'meshes':meshes,
         'materials':[{'name':'voxel_paint','pbrMetallicRoughness':{
            'baseColorTexture':{'index':0},'baseColorFactor':[1,1,1,1],
            'metallicFactor':0,'roughnessFactor':.9},'doubleSided':True}],
         'textures':[{'source':0,'sampler':0}],
         'samplers':[{'magFilter':9728,'minFilter':9728,'wrapS':33071,'wrapT':33071}],
         'images':[{'bufferView':glb.image(binary.getvalue()),'mimeType':'image/png'}],
         'animations':[animation_of(glb,period,keys,tracks)]}
    glb.write(doc,str(HERE/f'{name}-textured.glb'))


v2.Sculpture=ArtMesh
v2.box=box
v2.paint=lambda *args,**kwargs:None
v2.save=save
for style,fn in (('pup',v2.shadow_pup),('dragon',v2.ember_hatchling),('ghost',v2.cloud_wisp)):
    fn()
atlas.save(HERE/'pet-texture-atlas.png')
