package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import java.nio.file.Files;
import java.nio.file.Paths;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import net.minecraft.client.Minecraft;

/** Opt-in, two real clients on an offline local server; commands come from a private test file. */
final class NametagProof {
 private NametagProof() {}
 static void start() {
  Executors.newSingleThreadScheduledExecutor(r -> {
   Thread t = new Thread(r, "arctic-nametag-proof"); t.setDaemon(true); return t;
  }).scheduleAtFixedRate(() -> Minecraft.getInstance().execute(() -> tick()), 1, 1, TimeUnit.SECONDS);
 }
 @SuppressWarnings("unchecked")
 private static void tick() {
  Minecraft mc = Minecraft.getInstance();
  if (mc.level == null || mc.player == null) return;
  try {
   // Offline accounts cannot check in to the production membership service. Seed only the two
   // known offline UUIDs in its normal cache; badge selection and rendering are production code.
   java.lang.reflect.Field field = com.arcticlauncher.client.looks.Looks.class.getDeclaredField("players");
   field.setAccessible(true);
   java.util.Map<java.util.UUID, com.arcticlauncher.client.looks.Look> cache =
    (java.util.Map<java.util.UUID, com.arcticlauncher.client.looks.Look>) field.get(ArcticClient.looks());
   java.lang.reflect.Constructor<com.arcticlauncher.client.looks.Look> ctor =
    com.arcticlauncher.client.looks.Look.class.getDeclaredConstructor(String.class, boolean.class, String.class,
      java.util.List.class, boolean.class, long.class);
   ctor.setAccessible(true);
   for (String name : new String[]{"ArcticDev", "ArcticDev2"}) {
    java.util.UUID id = java.util.UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes(java.nio.charset.StandardCharsets.UTF_8));
    cache.put(id, ctor.newInstance(null, false, null, java.util.Collections.emptyList(), true, System.currentTimeMillis()));
   }
   mc.options.pauseOnLostFocus = false;
   Compat.setScreen(null);
   java.nio.file.Path commands = Paths.get(System.getProperty("arctic.proof.commands"));
   if (!Files.exists(commands)) return;
   String command = new String(Files.readAllBytes(commands), java.nio.charset.StandardCharsets.UTF_8).trim();
   Files.delete(commands);
   ArcticClient.config().freelookEnabled = true;
   ArcticClient.config().badgeNametags = true;
   ArcticClient.config().ownNametag = false;
   if (command.equals("normal")) {
    ArcticClient.features().simulate(false, false);
    ArcticClient.platform().setThirdPerson(false);
   } else if (command.equals("freelook")) {
    ArcticClient.features().simulate(false, true);
   } else if (command.equals("rotate")) {
    ArcticClient.features().turn(600, 0);
   } else if (command.equals("shot")) {
    WorldTest.shot("nametag-proof");
    System.out.println("nametag-proof: player=" + mc.player.getUUID() + " freelook=" + ArcticClient.features().freelook()
     + " cameraYaw=" + ArcticClient.features().lookYaw() + " body=" + Compat.yRot(mc.player));
   }
   System.out.println("nametag-proof: " + command);
  } catch (Exception e) { e.printStackTrace(); }
 }
}
