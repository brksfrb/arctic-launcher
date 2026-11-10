package com.arcticlauncher.legacy;

import com.arcticlauncher.client.ArcticClient;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.screen.ChatScreen;
import net.minecraft.text.LiteralText;
import net.minecraft.world.level.LevelInfo;
import net.minecraft.world.level.LevelGeneratorType;
import java.util.*;
import java.nio.file.*;

/** External, opt-in benchmark; never added to a release jar. */
public final class Fps189Benchmark {
    private static long started, stageAt, previous;
    private static int stage;
    private static final List<Long> samples = new ArrayList<>();
    private static void set(String key, boolean value) {
        try { ArcticClient.config().getClass().getField(key).setBoolean(ArcticClient.config(), value); }
        catch (ReflectiveOperationException ignored) {}
    }
    private static void command(String text) { MinecraftClient.getInstance().player.sendChatMessage("/" + text); }
    public static void frame() {
        if (!Boolean.getBoolean("fps189.bench")) return;
        long now = System.nanoTime();
        MinecraftClient mc = MinecraftClient.getInstance();
        if (started == 0) { started = now; System.out.println("FPS189 GL " + org.lwjgl.opengl.GL11.glGetString(org.lwjgl.opengl.GL11.GL_RENDERER)); }
        if (stage == 0 && now-started > 3_000_000_000L) {
            mc.options.pauseOnLostFocus = false;
            set("dynamicFps", false);
            set("fancy", !Boolean.getBoolean("fps189.plain")); set("fontAntialias", true);
            set("chatTimestamps", true); set("chatCopy", true); set("chatPersistent", true);
            set("chatStack", false); set("chatOff", false);
            set("connectedGlass", !Boolean.getBoolean("fps189.noctm"));
            set("chatPersistent", !Boolean.getBoolean("fps189.nopersist"));
            set("chatCopy", !Boolean.getBoolean("fps189.nocopy"));
            set("chatTimestamps", !Boolean.getBoolean("fps189.notimestamps"));
            // Disable replay capture identically; this bench diagnoses interactive rendering.
            set("replayEnabled", false);
            mc.startIntegratedServer("fps189-fixed", "fps189-fixed", new LevelInfo(20261108L, LevelInfo.GameMode.CREATIVE, false, false, LevelGeneratorType.FLAT).enableCommands());
            stage=1; stageAt=now;
        } else if (stage==1 && mc.world!=null && mc.player!=null && now-stageAt>8_000_000_000L) {
            mc.setScreen(null);
            command("gamerule doDaylightCycle false"); command("gamerule doMobSpawning false"); command("gamerule sendCommandFeedback false");
            command("time set 6000"); command("weather clear 1000000"); command("kill @e[type=!Player]");
            command("tp @p 0.5 100 0.5 0 0");
            mc.player.abilities.allowFlying=true; mc.player.abilities.flying=true;
            stage=2; stageAt=now;
        } else if(stage==2 && now-stageAt>8_000_000_000L) {
            command("fill -16 99 -16 16 99 24 stone");
            command("fill -8 100 8 8 103 8 glass");
            command("tp @p 0.5 100 0.5 0 0");
            for(int i=0;i<10;i++) {
                double x=(i%5-2)*1.5+.5, z=2.0+(i/5)*3;
                command("summon ArmorStand "+x+" 99.5 "+z+" {NoGravity:1b,CustomName:Player_"+i+",CustomNameVisible:1b}");
            }
            command("entitydata @e[type=ArmorStand] {Invisible:1b}");
            stage=8; stageAt=now;
        } else if(stage==8 && now-stageAt>8_000_000_000L) {
            mc.player.yaw=0; mc.player.pitch=0;
            mc.inGameHud.getChatHud().clear();
            for(int i=0;i<100;i++) mc.inGameHud.getChatHud().addMessage(new LiteralText(String.format("Line %03d <FPS189> Repeatable chat with timestamp and copy",i)));
            mc.setScreen(new ChatScreen(""));
            stage=3; stageAt=now;
        } else if(stage==3 && now-stageAt>20_000_000_000L) {
            samples.clear(); previous=0; stage=4; stageAt=now;
            System.out.println("FPS189 MEASURE open-chat");

        } else if(stage==4 || stage==6) {
            if(previous!=0) samples.add(now-previous);
            previous=now;
            if(now-stageAt>15_000_000_000L) {
                report(stage==4 ? "open-chat" : "closed-chat");
                if(stage==4) { mc.setScreen(null); stage=5; stageAt=now; }
                else { stage=7; mc.scheduleStop(); }
            }
        } else if(stage==5 && now-stageAt>5_000_000_000L) {
            // Keep 100 stored lines, refresh their age so the visible lines don't fade during sampling.
            mc.inGameHud.getChatHud().clear();
            for(int i=0;i<100;i++) mc.inGameHud.getChatHud().addMessage(new LiteralText(String.format("Line %03d <FPS189> Repeatable chat with timestamp and copy",i)));
            samples.clear(); previous=0; stage=6; stageAt=now;
            System.out.println("FPS189 MEASURE closed-chat");
        }
        if(now-started>180_000_000_000L) { System.out.println("FPS189 TIMEOUT stage="+stage); mc.scheduleStop(); }
    }
    private static void report(String scene) {
        long total=0; for(long v:samples) total+=v;
        Collections.sort(samples);
        String row=String.format(java.util.Locale.ROOT,"FPS189 RESULT %s n=%d mean_ms=%.4f median_ms=%.4f p95_ms=%.4f p99_ms=%.4f", scene,samples.size(),total/1e6/samples.size(),samples.get(samples.size()/2)/1e6,samples.get(samples.size()*95/100)/1e6,samples.get(samples.size()*99/100)/1e6);
        System.out.println(row);
        try { Files.write(Paths.get(System.getProperty("fps189.output")), (row+"\n").getBytes("UTF-8"),StandardOpenOption.CREATE,StandardOpenOption.APPEND); } catch(Exception e) { throw new RuntimeException(e); }
    }
}
