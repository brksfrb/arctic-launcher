package com.arcticlauncher.legacy;
public class Fps189Probe {
 private static final String[] NAMES={"font.prepare","font.glyph","ctm.begin","diagnostics.frame","persistent.restore","timestamp.text","copy.click","persistent.add"};
 private static final long[] count=new long[NAMES.length],time=new long[NAMES.length],at=new long[NAMES.length];
 private static Thread renderThread;
 private static final java.lang.management.ThreadMXBean CPU=java.lang.management.ManagementFactory.getThreadMXBean();
 private static long cpuStart;
 private static boolean active;private static int frames;
 private static java.lang.reflect.Field stage;
 public static void frame() {
  try {
   renderThread=Thread.currentThread();
   if(stage==null) {stage=Class.forName("com.arcticlauncher.legacy.Fps189Benchmark").getDeclaredField("stage");stage.setAccessible(true);}
   int s=stage.getInt(null);boolean on=s==4||s==6;
   if(on&&!active) {java.util.Arrays.fill(count,0);java.util.Arrays.fill(time,0);frames=0;cpuStart=CPU.getCurrentThreadCpuTime();}
   if(!on&&active) {
    System.out.printf(java.util.Locale.ROOT,"FPS189 PROBE client_cpu_ms_per_frame=%.6f%n",(CPU.getCurrentThreadCpuTime()-cpuStart)/1e6/frames);
    for(int i=0;i<NAMES.length;i++)System.out.printf(java.util.Locale.ROOT,"FPS189 PROBE %s calls=%d calls_per_frame=%.2f total_ms=%.3f ms_per_frame=%.6f%n",NAMES[i],count[i],count[i]/(double)frames,time[i]/1e6,time[i]/1e6/frames);
   }
   active=on;if(active)frames++;
  }catch(Exception e){throw new RuntimeException(e);}
 }
 public static void enter(int id) {if(active && Thread.currentThread()==renderThread){at[id]=System.nanoTime();count[id]++;}}
 public static void leave(int id) {if(active && Thread.currentThread()==renderThread)time[id]+=System.nanoTime()-at[id];}
}
