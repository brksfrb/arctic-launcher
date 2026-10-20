import * as THREE from 'three';
import {GLTFLoader} from 'three/examples/jsm/loaders/GLTFLoader.js';

const canvas=document.getElementById('view');
const renderer=new THREE.WebGLRenderer({canvas,antialias:true,powerPreference:'high-performance'});
renderer.setPixelRatio(Math.min(window.devicePixelRatio||1,1.5));
renderer.outputColorSpace=THREE.SRGBColorSpace;
renderer.toneMapping=THREE.ACESFilmicToneMapping;
renderer.toneMappingExposure=1.05;
const scene=new THREE.Scene();scene.background=new THREE.Color('#101927');
const camera=new THREE.PerspectiveCamera(33,1,.1,200);
scene.add(new THREE.HemisphereLight(0xe5f5ff,0x263041,1.6));
function light(color,intensity,x,y,z){const l=new THREE.DirectionalLight(color,intensity);l.position.set(x,y,z);scene.add(l)}
light(0xfff3df,1.8,-18,38,-28);light(0xb0d9ff,.9,22,29,18);light(0xffffff,.5,0,16,-20);
const player=new THREE.Group();scene.add(player);player.visible=false;
const mat=(hex)=>new THREE.MeshStandardMaterial({color:hex,roughness:1});
const skin=mat('#c89770'),hair=mat('#30271f'),shirt=mat('#267c92'),jacket=mat('#aa4445'),pants=mat('#343b4a'),shoe=mat('#23252b');
function cube(group,x,y,z,w,h,d,material){const mesh=new THREE.Mesh(new THREE.BoxGeometry(w,h,d),material);mesh.position.set(x,y,z);group.add(mesh);return mesh}
cube(player,-2,6,0,4,12,4,pants);cube(player,2,6,0,4,12,4,pants);
cube(player,-2,1,-.12,4,2,4.5,shoe);cube(player,2,1,-.12,4,2,4.5,shoe);
cube(player,0,18,0,8,12,4,shirt);
cube(player,-6,18,0,4,12,4,jacket);cube(player,6,18,0,4,12,4,jacket);
cube(player,-6,12,-.17,4,1.9,4.4,skin);cube(player,6,12,-.17,4,1.9,4.4,skin);
cube(player,0,28,0,8,8,8,skin);cube(player,0,31.55,.12,8.04,1.1,8.04,hair);
cube(player,-2.05,28.45,-4.08,1.17,1.45,.18,mat('#d6eced'));
cube(player,2.05,28.45,-4.08,1.17,1.45,.18,mat('#d6eced'));
cube(player,-2.05,28.30,-4.20,.36,.79,.12,mat('#183443'));
cube(player,2.05,28.30,-4.20,.36,.79,.12,mat('#183443'));

let current=null,mixer=null,clock=new THREE.Clock(),playing=true;
const loader=new GLTFLoader();
async function select(id){
  document.querySelectorAll('[data-pet]').forEach(b=>b.classList.toggle('active',b.dataset.pet===id));
  if(current)scene.remove(current);
  if(mixer)mixer.stopAllAction();
  const raw=atob(window.MODEL_DATA[id]);
  const bytes=new Uint8Array(raw.length);for(let i=0;i<raw.length;i++)bytes[i]=raw.charCodeAt(i);
  const gltf=await loader.parseAsync(bytes.buffer,'');
  current=gltf.scene;scene.add(current);
  current.traverse(obj=>{if(obj.isMesh){obj.material.side=THREE.DoubleSide;obj.material.roughness=.88;obj.material.metalness=0}});
  mixer=new THREE.AnimationMixer(current);
  if(gltf.animations.length)mixer.clipAction(gltf.animations[0]).play();
  document.getElementById('name').textContent=({shadow_pup:'Shadow Pup',ember_hatchling:'Ember Hatchling',cloud_wisp:'Cloud Wisp'})[id];
  targetX=id==='cloud_wisp'?-7.5:7.5;
}
for(const b of document.querySelectorAll('[data-pet]'))b.onclick=()=>select(b.dataset.pet);
let yaw=2.36, pitch=.10, distance=26,targetY=29.0,targetX=7.5,dragging=false,lastX=0,lastY=0;
function pose(y,p){yaw=y;pitch=p}
document.getElementById('front').onclick=()=>pose(Math.PI,.1);
document.getElementById('quarter').onclick=()=>pose(2.38,.14);
document.getElementById('side').onclick=()=>pose(Math.PI/2,.1);
document.getElementById('back').onclick=()=>pose(0,.1);
document.getElementById('motion').onclick=e=>{playing=!playing;e.target.textContent=playing?'Pause motion':'Play motion'};
document.getElementById('player').onclick=e=>{player.visible=!player.visible;e.target.textContent=player.visible?'Hide player':'Show player';distance=player.visible?54:26;targetY=player.visible?22.5:29};
canvas.addEventListener('pointerdown',e=>{dragging=true;lastX=e.clientX;lastY=e.clientY;canvas.setPointerCapture(e.pointerId)});
canvas.addEventListener('pointerup',()=>dragging=false);
canvas.addEventListener('pointermove',e=>{if(!dragging)return; yaw+=(e.clientX-lastX)*.008;pitch=Math.max(-.65,Math.min(.75,pitch+(e.clientY-lastY)*.006));lastX=e.clientX;lastY=e.clientY});
canvas.addEventListener('wheel',e=>{e.preventDefault();distance=Math.max(17,Math.min(90,distance+Math.sign(e.deltaY)*3))},{passive:false});
function frame(){requestAnimationFrame(frame);const w=canvas.clientWidth,h=canvas.clientHeight;if(canvas.width!==Math.round(w*renderer.getPixelRatio())||canvas.height!==Math.round(h*renderer.getPixelRatio())){renderer.setSize(w,h,false);camera.aspect=w/h;camera.updateProjectionMatrix()}
  const focusX=player.visible?0:targetX;
  camera.position.set(focusX+distance*Math.sin(yaw)*Math.cos(pitch),targetY+distance*Math.sin(pitch),distance*Math.cos(yaw)*Math.cos(pitch));camera.lookAt(focusX,targetY,0);
  const dt=Math.min(clock.getDelta(),.05);if(playing&&mixer)mixer.update(dt);renderer.render(scene,camera)}
const requested=new URLSearchParams(location.search).get('pet');
select(window.MODEL_DATA[requested]?requested:'shadow_pup').then(frame).catch(e=>{document.getElementById('error').textContent=e.message});
