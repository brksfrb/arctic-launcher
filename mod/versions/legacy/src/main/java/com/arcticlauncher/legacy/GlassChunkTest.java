//#if MC < 1.9
package com.arcticlauncher.legacy;

import java.lang.reflect.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicIntegerArray;
import net.minecraft.util.math.BlockPos;
import net.minecraft.util.math.Direction;
import net.minecraft.block.BlockState;
import net.minecraft.world.BlockView;
import net.minecraft.client.MinecraftClient;
import com.arcticlauncher.client.ctm.GlassTiles;

/** Opt-in real chunk-cache / packet-load regression: -Pdevelopment_scenes=true -Darctic.selftest=ctmchunks. */
final class GlassChunkTest {
 private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
  Thread t = new Thread(r, "arctic-chunk-proof"); t.setDaemon(true); return t;
 });
 private static final AtomicIntegerArray calls = new AtomicIntegerArray(4), masks = new AtomicIntegerArray(4);
 private static volatile boolean watch;
 private static int[] before;
 private static net.minecraft.network.packet.s2c.play.ChunkDataS2CPacket packet;
 private GlassChunkTest() {}
 static void start() { later(10, () -> LegacyWorldTest.createTestWorld(() -> { command("tp @p 15.5 224 -16 0 0"); later(7, GlassChunkTest::scene); })); }
 private static void later(int seconds, Runnable r) { TIMER.schedule(() -> LegacyHooks.onNextFrame(r), seconds, TimeUnit.SECONDS); }
 private static void command(String c) { LegacyWorldTest.command(c); }
 private static void check(boolean b, String label) { if (!b) throw new AssertionError("ctmchunks: " + label); }
 private static int slot(BlockPos p) {
  if (p.getX() != 15) return -1;
  if (p.getZ() == 15 && p.getY() == 232) return 3;
  if (p.getZ() != 0) return -1;
  return p.getY() == 221 ? 0 : p.getY() == 225 ? 1 : p.getY() == 229 ? 2 : -1;
 }
 static boolean watching(BlockPos p) { return watch && slot(p) >= 0; }
 static void observe(BlockPos p, Object view, int mask) {
  int i = slot(p); masks.set(i, mask); calls.incrementAndGet(i);
  ArcticLegacy.LOG.info("ctmchunks mesh slot={} mask={} view={}", i, mask, view.getClass().getName());
 }
 private static void scene() {
  command("gamemode creative"); command("fill 8 218 -2 23 233 17 minecraft:air");
  command("fill 8 219 -18 23 219 17 minecraft:stone");
  command("fill 10 220 0 21 222 0 minecraft:glass");
  command("fill 10 224 0 21 226 0 minecraft:stained_glass 14");
  command("fill 10 228 0 21 230 0 minecraft:glass_pane");
  command("fill 14 232 14 16 232 16 minecraft:glass");
  command("fill 14 223 -18 16 223 -14 minecraft:stone");
  command("fill 14 236 8 16 236 10 minecraft:stone");
  command("setblock 23 225 0 minecraft:stained_glass 11");
  command("time set 6000"); command("weather clear"); command("tp @p 15.5 224 -16 0 0");
  com.arcticlauncher.client.ArcticClient.config().connectedGlass = true;
  MinecraftClient.getInstance().options.hudHidden = true;
  watch = true;
  later(1, () -> awaitMeshes(0));
 }
 private static void awaitMeshes(int attempt) {
  for (int i=0;i<3;i++) if (calls.get(i)==0 || masks.get(i)!=255) {
   check(attempt<120,"initial terrain did not finish meshing slot="+i);
   later(1, () -> awaitMeshes(attempt+1));return;
  }
  later(3, GlassChunkTest::unload);
 }
 private static void unload() {
  MinecraftClient mc = MinecraftClient.getInstance();
  // This first check uses the renderer's own BlockView, not a world substituted for it.
  for (int i=0;i<3;i++) { check(calls.get(i)>0, "already-loaded border was meshed " + i); check(masks.get(i)==255, "already-loaded neighbour visible " + i); }
  watch=false;
  try { topology(); } finally { watch=true; }
  packet = new net.minecraft.network.packet.s2c.play.ChunkDataS2CPacket(mc.world.getChunk(1,0),true,65535);
  before = new int[]{calls.get(0), calls.get(1), calls.get(2), calls.get(3)};
  mc.world.handleChunk(1,0,false);
  later(6, () -> {
   unloadedView(false);
   LegacyWorldTest.shot("ctmchunks-unloaded");
   before = new int[]{calls.get(0), calls.get(1), calls.get(2), calls.get(3)};
   mc.getNetworkHandler().onChunkData(packet);
   later(8, () -> {
    for (int i=0;i<3;i++) { check(calls.get(i)>before[i], "packet load rebuilt border " + i); check(masks.get(i)==255, "loaded east reconnects " + i); }
    LegacyWorldTest.shot("ctmchunks-loaded");
    ArcticLegacy.LOG.info("ctmchunks PASS: glass, stained glass, panes; real meshing views; unloaded then loaded; no manual reload");
    diagonal();
   });
  });
 }
 private static void diagonal() {
  MinecraftClient mc = MinecraftClient.getInstance();
  command("tp @p 15.5 237 9 0 55");
  packet = new net.minecraft.network.packet.s2c.play.ChunkDataS2CPacket(mc.world.getChunk(1,1),true,65535);
  mc.world.handleChunk(1,1,false);
  later(6, () -> {
   unloadedView(true);
   LegacyWorldTest.shot("ctmchunks-diagonal-unloaded");
   int count = calls.get(3);
   mc.getNetworkHandler().onChunkData(packet);
   later(8, () -> {
    check(calls.get(3)>count,"diagonal packet load rebuilt corner");
    check(masks.get(3)==255,"loaded diagonal restores the inner corner");
    LegacyWorldTest.shot("ctmchunks-diagonal-loaded");
    ArcticLegacy.LOG.info("ctmchunks PASS: diagonal chunk load restores corner with no manual reload");
    later(2, () -> mc.scheduleStop());
   });
  });
 }
 /** A fresh game meshing cache must expose an absent neighbour as air. */
 private static void unloadedView(boolean diagonal) {
  MinecraftClient mc = MinecraftClient.getInstance();
  BlockView view = new net.minecraft.client.world.ChunkRenderCache(mc.world,new BlockPos(-1,207,-1),new BlockPos(16,240,16),1);
  watch=false;
  try {
   Field local=LegacyGlassCtm.class.getDeclaredField("LOCAL"); local.setAccessible(true);
   Object c=((ThreadLocal<?>)local.get(null)).get();
   Method mask=c.getClass().getDeclaredMethod("mask",Direction.class);mask.setAccessible(true);
   for(int y:diagonal ? new int[]{232} : new int[]{221,225,229}) {
    BlockPos p=new BlockPos(15,y,diagonal ? 15 : 0);
    LegacyGlassCtm.begin(view,mc.world.getBlockState(p),p);
    check((Integer)mask.invoke(c,diagonal ? Direction.UP : Direction.NORTH)==(diagonal ? 247 : 124),"unloaded cache mask y="+y);
   }
   ArcticLegacy.LOG.info("ctmchunks PASS: unloaded neighbour reads as air in fresh meshing view");
  } catch(ReflectiveOperationException e) { throw new AssertionError(e); }
  finally { LegacyGlassCtm.end();watch=true; }
 }
 /** Exhaust all faces and occupancy masks at positive and negative chunk borders. */
 private static void topology() {
  MinecraftClient mc = MinecraftClient.getInstance();
  BlockState glass = mc.world.getBlockState(new BlockPos(15,221,0));
  BlockState stained = mc.world.getBlockState(new BlockPos(15,225,0));
  BlockState pane = mc.world.getBlockState(new BlockPos(15,229,0));
  BlockState air = mc.world.getBlockState(new BlockPos(15,233,0));
  try {
   Field local = LegacyGlassCtm.class.getDeclaredField("LOCAL"); local.setAccessible(true);
   Object context = ((ThreadLocal<?>) local.get(null)).get();
   Method mask = context.getClass().getDeclaredMethod("mask", Direction.class); mask.setAccessible(true);
   // Explicit face bases avoid relying on enum ordering or the implementation's switch.
   int assertions=0;
   java.util.Set<Integer> tiles=new java.util.HashSet<>();
   for (BlockState state : new BlockState[]{glass,stained,pane}) for (Direction face : Direction.values()) for (int border : new int[]{-17,-1,15,31}) for (int raw=0;raw<256;raw++) {
    int rx=0,rz=0,by=-1,bz=0;
    switch(face) { case NORTH:rx=-1;break;case SOUTH:rx=1;break;case WEST:rz=1;break;case EAST:rz=-1;break;case UP:rx=1;by=0;bz=1;break;case DOWN:rx=1;by=0;bz=-1;break; }
    final int[] dx={-rx,-rx,0,rx,rx,rx,0,-rx}, dy={0,by,by,by,0,-by,-by,-by}, dz={-rz,-rz+bz,bz,rz+bz,rz,rz-bz,-bz,-rz-bz};
    final int bits=raw, origin=border;
    BlockView view=(BlockView)Proxy.newProxyInstance(BlockView.class.getClassLoader(),new Class<?>[]{BlockView.class},(proxy,method,args)->{
     if (args != null && args.length == 1 && args[0] instanceof BlockPos && method.getReturnType() == BlockState.class) {
      BlockPos p=(BlockPos)args[0];
      for(int n=0;n<8;n++) if(p.getX()==origin+dx[n] && p.getY()==221+dy[n] && p.getZ()==origin+dz[n]) return (bits&(1<<n))!=0 ? state : air;
      return air;
     }
     throw new UnsupportedOperationException(method.toString());
    });
    LegacyGlassCtm.begin(view,state,new BlockPos(border,221,border));
    int expected=raw&85;
    for(int n=1;n<8;n+=2) { int a=1<<(n-1), b=1<<((n+1)%8); if((raw&(a|b|(1<<n)))==(a|b|(1<<n))) expected|=1<<n; }
    int actual=(Integer)mask.invoke(context,face);
    check(actual==expected,"topology " + face + " border="+border+" raw="+raw);
    check(GlassTiles.index(actual)>=0 && GlassTiles.index(actual)<47,"47-tile index"); tiles.add(GlassTiles.index(actual)); assertions++;
    LegacyGlassCtm.end();
   }
   check(tiles.size()==47,"all 47 tiles covered");
   BlockState blue=mc.world.getBlockState(new BlockPos(23,225,0));
   for(BlockState other:new BlockState[]{glass,pane,blue}) {
    BlockView wrong=(BlockView)Proxy.newProxyInstance(BlockView.class.getClassLoader(),new Class<?>[]{BlockView.class},(proxy,method,args)->other);
    LegacyGlassCtm.begin(wrong,stained,new BlockPos(15,225,0));
    check((Integer)mask.invoke(context,Direction.NORTH)==0,"different block or stained color does not connect");
    LegacyGlassCtm.end();
   }
   ArcticLegacy.LOG.info("ctmchunks PASS: {} actual Context topology checks, all 47 tiles, block/color isolation",assertions);
  } catch(ReflectiveOperationException e) { throw new AssertionError(e); }
 }
}

//#endif
