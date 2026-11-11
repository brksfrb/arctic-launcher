package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.feature.Badge;
import com.arcticlauncher.client.menu.Menus;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.charset.StandardCharsets;
import java.util.UUID;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;

/** Opt-in real clients using the actual looks service: no fabricated Arctic presence or styles. */
final class BadgeTest {
    private static final ScheduledExecutorService TIMER = Executors.newSingleThreadScheduledExecutor(r -> {
        Thread t = new Thread(r, "arctic-badge-proof"); t.setDaemon(true); return t;
    });
    private static final boolean PEER = Boolean.getBoolean("arctic.badge.peer");
    private static final Path PROOF = Paths.get(System.getProperty("arctic.badge.proof"));
    private static final Path PORT = PROOF.resolve("1.21.11-lan-port.txt");
    private static final Path SCENE = PROOF.resolve("1.21.11-scene-ready");
    private static final String PREFIX = "1.21.11-" + (PEER ? "B" : "A");
    private static final String HOST_NAME = "BadgeIceM";
    private static final String PEER_NAME = "BadgeLineM";
    private static final UUID OTHER = UUID.nameUUIDFromBytes(("OfflinePlayer:" + (PEER ? HOST_NAME : PEER_NAME)).getBytes(StandardCharsets.UTF_8));
    private BadgeTest() {}
    static boolean requested() { return "badge".equals(System.getProperty("arctic.selftest")); }
    private static void run(Runnable action) { Minecraft.getInstance().execute(action); }
    private static void later(int seconds, Runnable action) { TIMER.schedule(() -> run(action), seconds, TimeUnit.SECONDS); }
    private static void command(String command) { Compat.sendChat("/" + command.replace("BadgeIce", HOST_NAME).replace("BadgeLine", PEER_NAME)); }
    private static void tab(boolean on) { Compat.setDown(Minecraft.getInstance().options.keyPlayerList, on); }
    private static void camera(boolean front) {
        //#if MC >= 1.16
        Minecraft.getInstance().options.setCameraType(front ? net.minecraft.client.CameraType.THIRD_PERSON_FRONT : net.minecraft.client.CameraType.FIRST_PERSON);
        //#else
        Minecraft.getInstance().options.thirdPersonView = front ? 2 : 0;
        //#endif
    }
    private static void log(String text) { ArcticMod.LOG.info("badgeproof: " + text); }

    @SuppressWarnings("unchecked")

    // Solo scene (-Darctic.badge.solo=true -Darctic.badge.label=x): own name tag and player list in each style, no server.
    private static final UUID OTHER_SOLO = UUID.nameUUIDFromBytes(("OfflinePlayer:BadgeLine" + (false ? "" : "M")).getBytes(StandardCharsets.UTF_8));
    private static final String LABEL = System.getProperty("arctic.badge.label", "run");
    private static void seed(int style) {
        try {
            java.lang.reflect.Field field = com.arcticlauncher.client.looks.Looks.class.getDeclaredField("players");
            field.setAccessible(true);
            java.util.Map<UUID, com.arcticlauncher.client.looks.Look> cache = (java.util.Map<UUID, com.arcticlauncher.client.looks.Look>) field.get(ArcticClient.looks());
            java.lang.reflect.Constructor<com.arcticlauncher.client.looks.Look> ctor = com.arcticlauncher.client.looks.Look.class.getDeclaredConstructor(
                String.class, boolean.class, String.class, java.util.List.class, boolean.class, long.class, java.util.Map.class, int.class);
            ctor.setAccessible(true);
            for (UUID who : new UUID[] {ArcticClient.platform().worldPlayerId(), OTHER_SOLO}) cache.put(who, ctor.newInstance(null, false, null, java.util.Collections.emptyList(), true,
                System.currentTimeMillis() + 3600000L, java.util.Collections.emptyMap(), style));
        } catch (Exception e) { throw new RuntimeException(e); }
    }
    private static void soloStep(int step) {
        int style = step / 3, kind = step % 3;
        if (style >= Badge.STYLES.size()) { fov(70); log("DONE"); Minecraft.getInstance().stop(); return; }
        seed(style);
        later(1, () -> {
            seed(style);
            if (kind == 0) { tab(false); camera(true); fov(70); }
            if (kind == 1) { fov(30); }
            if (kind == 2) { fov(70); camera(false); tab(true); }
        });
        later(4, () -> { seed(style); soloShot("style" + style + "-" + new String[] {"normal", "close", "tab"}[kind]); });
        later(6, () -> soloStep(step + 1));
    }
    private static void fov(int degrees) { Minecraft.getInstance().options.fov().set(degrees); }
    private static void soloShot(String suffix) {
        Minecraft mc = Minecraft.getInstance();
        net.minecraft.client.Screenshot.takeScreenshot(mc.getMainRenderTarget(), image -> {
            try { image.writeToFile(PROOF.resolve("1.21.11-" + LABEL + "-" + suffix + ".png")); }
            catch (Exception e) { throw new RuntimeException(e); }
            finally { image.close(); }
        });
        log("screenshot " + suffix);
    }

    private static void lanThenSteps() {
        if (!Boolean.getBoolean("arctic.badge.lan")) { soloStep(0); return; }
        Minecraft mc = Minecraft.getInstance();
        int port = net.minecraft.util.HttpUtil.getAvailablePort();
        mc.getSingleplayerServer().setUsesAuthentication(false);
        mc.getSingleplayerServer().publishServer(net.minecraft.world.level.GameType.CREATIVE, true, port);
        try { Files.write(PORT, Integer.toString(port).getBytes(StandardCharsets.UTF_8)); } catch (Exception e) { throw new RuntimeException(e); }
        waitSecond(0);
    }
    private static void waitSecond(int tries) {
        Minecraft mc = Minecraft.getInstance();
        if (mc.getConnection().getOnlinePlayers().size() >= 2) { later(5, () -> soloStep(0)); }
        else if (tries < 240) later(1, () -> waitSecond(tries + 1));
        else throw new IllegalStateException("peer never joined");
    }
    private static void soloPeer() {
        later(10, () -> {
            Minecraft.getInstance().options.pauseOnLostFocus = false;
            try { ArcticClient.platform().joinSaved("Badge proof", "127.0.0.1:" + new String(Files.readAllBytes(PORT), StandardCharsets.UTF_8).trim(), false); }
            catch (Exception e) { throw new RuntimeException(e); }
            later(150, () -> Minecraft.getInstance().stop());
        });
    }
    private static void solo() {
        later(10, () -> {
            Minecraft mc = Minecraft.getInstance(); mc.options.pauseOnLostFocus = false;
            ClientConfig c = ArcticClient.config(); c.fancy = false; c.badgeNametags = true;
            c.badgeOthersOnly = false; c.badgeWhere = 0; c.ownNametag = true;
            WorldTest.createTestWorld(() -> {
                command("gamerule doDaylightCycle false"); command("time set 6000"); command("weather clear");
                command("tp BadgeIce 0.5 120 0.5 0 20");
                later(3, () -> { command("fill -5 119 -5 5 119 9 stone"); command("tp BadgeIce 0.5 120 0.5 0 20"); });
                later(7, () -> lanThenSteps());
            });
        });
    }

    static void start() {
        if (Boolean.getBoolean("arctic.badge.solo")) { if (PEER) soloPeer(); else solo(); return; }
        later(10, () -> {
            Minecraft mc = Minecraft.getInstance(); mc.options.pauseOnLostFocus = false;
            ClientConfig c = ArcticClient.config(); c.fancy = false; c.badgeNametags = true;
            c.badgeOthersOnly = false; c.badgeWhere = 0; c.ownNametag = true;
            if (PEER) {
                try { ArcticClient.platform().joinSaved("Badge proof", "127.0.0.1:" + new String(Files.readAllBytes(PORT), StandardCharsets.UTF_8).trim(), false); }
                catch (Exception e) { throw new RuntimeException(e); }
                waitPeer(0);
            } else { WorldTest.createTestWorld(BadgeTest::hostScene); }
        });
    }

    private static void hostScene() {
        command("gamerule doDaylightCycle false"); command("time set 6000"); command("weather clear");
        command("tp BadgeIce 0.5 120 0.5 0 20");
        later(3, () -> {
            command("fill -5 119 -5 5 119 9 stone");
            command("tp BadgeIce 0.5 120 0.5 0 20");
        });
        command("scoreboard objectives add arctic dummy"); command("scoreboard objectives setdisplay list arctic");
        camera(true);
        // Compare the same default badge, camera and GUI scale as the before run.
        later(14, () -> { tab(true); });
        later(17, () -> shot("after-tab"));
        later(18, () -> tab(false));
        later(21, () -> shot("after-nametag"));
        later(22, () -> ArcticClient.looks().wearBadgeStyle(1));
        later(25, () -> {
            Minecraft mc = Minecraft.getInstance();
            int port = net.minecraft.util.HttpUtil.getAvailablePort();
            mc.getSingleplayerServer().setUsesAuthentication(false);
            mc.getSingleplayerServer().publishServer(net.minecraft.world.level.GameType.CREATIVE, true, port);
            try { Files.write(PORT, Integer.toString(port).getBytes(StandardCharsets.UTF_8)); } catch (Exception e) { throw new RuntimeException(e); }
            log("LAN ready " + port);
            camera(false);
            waitWearers(0);
        });
    }

    private static void waitPeer(int tries) {
        Minecraft mc = Minecraft.getInstance();
        if (mc.level != null && mc.player != null) { ArcticClient.looks().wearBadgeStyle(2); waitWearers(0); }
        else if (tries < 180) later(1, () -> waitPeer(tries + 1));
        else throw new IllegalStateException("peer never joined");
    }

    private static void waitWearers(int tries) {
        Minecraft mc = Minecraft.getInstance();
        boolean present = mc.level != null && mc.player != null && mc.getConnection().getOnlinePlayers().size() >= 2;
        int other = ArcticClient.looks().badgeStyle(OTHER);
        boolean arctic = ArcticClient.looks().isArctic(OTHER);
        if (present && arctic && other == (PEER ? 1 : 2) && (!PEER || Files.exists(SCENE))) {
            log("two real wearers: own=" + ArcticClient.looks().badgeStyle(ArcticClient.platform().worldPlayerId()) + ", other=" + other);
            if (!PEER) {
                command("gamemode creative BadgeIce"); command("gamemode creative BadgeLine");
                command("tp BadgeIce 0.5 120 0.5 0 -8"); command("tp BadgeLine 0.5 120 4.5 180 -8");
                command("effect BadgeIce clear"); command("effect BadgeLine clear");
                try { Files.write(SCENE, new byte[] {1}); } catch (Exception e) { throw new RuntimeException(e); }
            }
            later(5, () -> { camera(false); Compat.setScreen(null); tab(true); });
            later(10, () -> shot("sees-other-tab"));
            later(11, () -> tab(false));
            later(15, () -> shot("sees-other-nametag"));
            later(17, () -> Menus.open("badgetags"));
            later(21, () -> shot("picker"));
            later(25, () -> { Compat.setScreen(null); camera(true); });
            later(29, () -> shot("own-nametag"));
            later(38, () -> { log("DONE"); mc.stop(); });
        } else if (tries < 180) later(1, () -> waitWearers(tries + 1));
        else throw new IllegalStateException("wearer sync failed: present=" + present + ", arctic=" + arctic + ", other=" + other);
    }

    private static void shot(String suffix) {
        Minecraft mc = Minecraft.getInstance();
        net.minecraft.client.Screenshot.takeScreenshot(mc.getMainRenderTarget(), image -> {
            try { image.writeToFile(PROOF.resolve(PREFIX + "-" + suffix + ".png")); }
            catch (Exception e) { throw new RuntimeException(e); }
            finally { image.close(); }
        });
        log("screenshot " + PREFIX + "-" + suffix);
    }
}
