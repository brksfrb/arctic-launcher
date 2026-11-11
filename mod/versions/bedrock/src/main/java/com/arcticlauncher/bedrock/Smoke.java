package com.arcticlauncher.bedrock;

import java.io.File;
import java.awt.image.BufferedImage;
import javax.imageio.ImageIO;

/** Offline shared-client render check; does not pretend to validate Bedrock engine hooks. */
public final class Smoke {
    public static void main(String[] args) throws Exception {
        File out = new File(args[0]);
        out.mkdirs();
        System.setProperty("arctic.cosmetics.url", "http://127.0.0.1:9");
        File config = java.nio.file.Files.createTempDirectory("arctic-bedrock-offline-").toFile();
        java.nio.file.Path probe=java.nio.file.Files.createTempDirectory("arctic-bedrock-frame-probe-");
        System.setProperty("arctic.bedrock.probe.directory",probe.toString());
        NativeBridge.init(config.getAbsolutePath(), "test", "ArcticTest");
        if (!"ArcticTest".equals(com.arcticlauncher.client.ArcticClient.platform().playerName())) {
            throw new AssertionError("headless test did not select the offline profile");
        }
        if (com.arcticlauncher.client.ArcticClient.platform().worldPlayerId() != null) {
            throw new AssertionError("menu preview fabricated an in-world identity");
        }
        NativeBridge.frame(800, 450, 144, 300, 200, new boolean[256], new int[] {0, 0xA1});
        Thread.sleep(220); // Let the shared menu's opening animation finish.
        byte[] pixels = NativeBridge.frame(800, 450, 144, 300, 200, new boolean[256], new int[0]);
        if (!NativeBridge.menuOpen()) throw new AssertionError("Right Shift did not open the shared client menu");
        BufferedImage image = new BufferedImage(800, 450, BufferedImage.TYPE_INT_ARGB);
        int opaque = 0;
        for (int i = 0; i < 800 * 450; i++) {
            int rgba = ((pixels[i * 4 + 3] & 255) << 24) | ((pixels[i * 4] & 255) << 16)
                    | ((pixels[i * 4 + 1] & 255) << 8) | (pixels[i * 4 + 2] & 255);
            image.setRGB(i % 800, i / 800, rgba);
            if ((rgba >>> 24) != 0) opaque++;
        }
        if (opaque < 1000) throw new AssertionError("shared menu produced an empty surface");
        ImageIO.write(image, "PNG", new File(out, "menu.png"));
        NativeBridge.frame(800, 450, 144, 0, 0, new boolean[256], new int[] {0, 27});
        if (NativeBridge.menuOpen()) throw new AssertionError("Escape did not close the menu");
        String world="{\"schema\":1,\"width\":16,\"height\":16,\"session\":\"offline-smoke\",\"sequence\":1,\"clockMs\":0,\"dimension\":\"overworld\",\"viewProjection\":[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1],\"camera\":[0,0,0,0,0,70],\"players\":[]}";
        float[] depth=new float[16*16];java.util.Arrays.fill(depth,1);
        NativeBridge.worldFrame(world,depth);
        java.nio.file.Files.write(probe.resolve("probe-world.request.json"),"{\"sequence\":17}".getBytes("UTF-8"));
        NativeBridge.frame(16,16,60,0,0,new boolean[256],new int[0]);
        if(NativeBridge.frameProbeSequence()!=17 || !java.nio.file.Files.exists(probe.resolve("probe-world-17.png")))
            throw new AssertionError("returned pixels lost their probe layer marker");
        NativeBridge.frame(16,16,60,0,0,new boolean[256],new int[0]);
        if(NativeBridge.frameProbeSequence()!=0)throw new AssertionError("probe marker leaked into the next frame");
        java.nio.file.Files.write(probe.resolve("probe-world.request.json"),"{\"sequence\":17}".getBytes("UTF-8"));
        NativeBridge.frame(16,16,60,0,0,new boolean[256],new int[0]);
        if(NativeBridge.frameProbeSequence()!=0 || !java.nio.file.Files.exists(probe.resolve("probe-world-error.json")))
            throw new AssertionError("a rejected probe request acquired a frame marker");
        NativeBridge.worldFrame(null,null);
        System.out.println("Shared-client menu/input/raster smoke passed: " + out.getAbsolutePath());
        System.out.println("Returned-pixel probe markers passed: captured, reset, stale request rejected");
        System.exit(0);
    }
}
