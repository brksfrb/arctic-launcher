package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.gfx.Gfx;
import java.awt.*;
import java.awt.image.BufferedImage;
import java.io.InputStream;
import java.util.ArrayDeque;
import java.util.Deque;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import javax.imageio.ImageIO;

/** Rasterizes the unchanged shared client UI for upload to the game's swap chain. */
public final class SoftwareGfx implements Gfx, AutoCloseable {
    private final BufferedImage image;
    private final int logicalWidth,logicalHeight,guiScale;
    private Graphics2D g;
    private final Deque<Graphics2D> stack = new ArrayDeque<Graphics2D>();
    private Shape clip;
    private final Map<String, BufferedImage> textures;
    private final Map<String, Boolean> missing = new ConcurrentHashMap<String, Boolean>();
    private static final Font FONT = new Font(Font.SANS_SERIF, Font.PLAIN, 10);
    private BedrockPlatform platform;
    private static final int[] COLORS = {
        0x000000, 0x0000AA, 0x00AA00, 0x00AAAA, 0xAA0000, 0xAA00AA, 0xFFAA00, 0xAAAAAA,
        0x555555, 0x5555FF, 0x55FF55, 0x55FFFF, 0xFF5555, 0xFF55FF, 0xFFFF55, 0xFFFFFF
    };

    public SoftwareGfx(int width, int height, Map<String, BufferedImage> textures) {
        this(width,height,textures,1);
    }
    private SoftwareGfx(int width,int height,Map<String,BufferedImage> textures,int guiScale) {
        if (width < 1 || height < 1 || width > 3840 || height > 2160 || guiScale<1 || guiScale>4) {
            throw new IllegalArgumentException("invalid surface dimensions");
        }
        this.image = new BufferedImage(width, height, BufferedImage.TYPE_INT_ARGB);
        this.guiScale=guiScale;logicalWidth=(width+guiScale-1)/guiScale;logicalHeight=(height+guiScale-1)/guiScale;
        this.textures = textures;
        g = image.createGraphics();
        g.scale(guiScale,guiScale);
        g.setFont(FONT);
        g.setRenderingHint(RenderingHints.KEY_TEXT_ANTIALIASING, RenderingHints.VALUE_TEXT_ANTIALIAS_ON);
        g.setRenderingHint(RenderingHints.KEY_INTERPOLATION, RenderingHints.VALUE_INTERPOLATION_NEAREST_NEIGHBOR);
    }
    public SoftwareGfx(int width,int height,BedrockPlatform platform) {
        this(width,height,platform.textures);this.platform=platform;
    }
    public SoftwareGfx(int width,int height,BedrockPlatform platform,int guiScale) {
        this(width,height,platform.textures,guiScale);this.platform=platform;
    }
    void image(BufferedImage image,int x,int y) { g.drawImage(image,x,y,null); }

    /** Copy world pixels before drawing the scaled GUI. This path has no GUI
     * transform or interpolation, and retains unpremultiplied ARGB exactly. */
    void worldImage(BufferedImage layer) {
        if(layer==null||layer.getWidth()!=image.getWidth()||layer.getHeight()!=image.getHeight())
            throw new IllegalArgumentException("world layer dimensions differ from its pixel surface");
        int[] pixels=((java.awt.image.DataBufferInt)image.getRaster().getDataBuffer()).getData();
        layer.getRGB(0,0,image.getWidth(),image.getHeight(),pixels,0,image.getWidth());
    }

    public BufferedImage image() { return image; }
    @Override public int width() { return logicalWidth; }
    @Override public int height() { return logicalHeight; }
    @Override public float pixelScale() { return guiScale; }
    @Override public void fill(int x0, int y0, int x1, int y1, int color) {
        g.setColor(new Color(color, true));
        g.fillRect(x0, y0, x1 - x0, y1 - y0);
    }
    @Override public void gradient(int x0, int y0, int x1, int y1, int top, int bottom) {
        if (y0 == y1) return;
        g.setPaint(new GradientPaint(x0, y0, new Color(top, true), x0, y1, new Color(bottom, true)));
        g.fillRect(x0, y0, x1 - x0, y1 - y0);
    }
    @Override public boolean roundedFill(int x0, int y0, int x1, int y1, int radius, int color) {
        g.setColor(new Color(color, true));
        g.fillRoundRect(x0, y0, x1 - x0, y1 - y0, radius * 2, radius * 2);
        return true;
    }
    @Override public void text(String text, int x, int y, int color, boolean shadow) {
        if (text == null || (color >>> 24) == 0) return;
        if (shadow) drawText(text, x + 1, y + 1, color, true);
        drawText(text, x, y, color, false);
    }
    private void drawText(String text, int x, int y, int original, boolean shadow) {
        int color = original;
        for (int i = 0; i < text.length();) {
            int cp = text.codePointAt(i);
            i += Character.charCount(cp);
            if (cp == 0xA7 && i < text.length()) {
                char code = Character.toLowerCase(text.charAt(i++));
                int index = "0123456789abcdef".indexOf(code);
                if (index >= 0) color = (original & 0xFF000000) | COLORS[index];
                if (code == 'r') color = original;
                continue;
            }
            int rgb = shadow ? (color & 0xFF000000) | ((color & 0xFCFCFC) >> 2) : color;
            g.setColor(new Color(rgb, true));
            String value = new String(Character.toChars(cp));
            g.drawString(value, x, y + 9);
            x += g.getFontMetrics().stringWidth(value);
        }
    }
    @Override public int textWidth(String text) {
        if (text == null) return 0;
        return g.getFontMetrics().stringWidth(text.replaceAll("\u00a7.", ""));
    }
    private BufferedImage texture(String key) {
        BufferedImage found = textures.get(key);
        if (found != null || missing.containsKey(key)) return found;
        String resource = "icon".equals(key) ? "/assets/arctic/icon.png"
                : key.startsWith("asset:") ? "/assets/arctic/" + key.substring(6) + ".png" : null;
        if (resource == null) return null; // Downloaded textures may arrive later.
        try (InputStream in = getClass().getResourceAsStream(resource)) {
            if (in != null) found = ImageIO.read(in);
        } catch (Exception ignored) { }
        if (found == null) missing.put(key, true); else textures.put(key, found);
        return found;
    }
    @Override public void texture(String key, int x, int y, int w, int h, float u, float v,
            int regionW, int regionH, int texW, int texH) {
        BufferedImage texture = texture(key);
        if (texture == null || texW <= 0 || texH <= 0) return;
        double sx = texture.getWidth() / (double) texW;
        double sy = texture.getHeight() / (double) texH;
        g.drawImage(texture, x, y, x + w, y + h, (int) (u * sx), (int) (v * sy),
                (int) ((u + regionW) * sx), (int) ((v + regionH) * sy), null);
    }
    @Override public void item(Object stack, int x, int y) {
        BufferedImage icon=itemImage(stack);if(icon==null)return;
        drawItem(icon,x,y);
        WorldFrame.ItemSlot slot=(WorldFrame.ItemSlot)stack;
        if(slot.count!=null&&slot.count>1) {
            String count=String.valueOf(slot.count);text(count,x+17-textWidth(count),y+9,0xffffffff,true);
        }
    }
    @Override public void itemIcon(Object stack,int x,int y) {
        BufferedImage icon=itemImage(stack);if(icon!=null)drawItem(icon,x,y);
    }
    private BufferedImage itemImage(Object stack) {
        return platform!=null&&stack instanceof WorldFrame.ItemSlot?platform.itemSprites.image((WorldFrame.ItemSlot)stack):null;
    }
    private void drawItem(BufferedImage icon,int x,int y) {
        g.drawImage(icon,x,y,x+16,y+16,0,0,icon.getWidth(),icon.getHeight(),null);
    }
    // Other sprites and 3D/dynamic/glint/durability item passes need their own
    // observed native bindings; static item PNGs do not attest those passes.
    @Override public void sprite(Object sprite, int x, int y, int w, int h) { }
    @Override public void player(int x0, int y0, int x1, int y1, int scale, int mx, int my) {
        if(platform==null || !platform.inWorld() || x1<=x0 || y1<=y0) return;
        WorldFrame.Player local=platform.world.local();
        AvatarRenderer.Avatar avatar=platform.avatar(java.util.UUID.fromString(local.id),platform.world.cosmeticTimeMillis());
        if(!platform.replayAppearanceActive())avatar.layerVisibility=local.layerVisibility;
        if(avatar.skin==null) {
            avatar.slim=local.nativeSlim!=null?local.nativeSlim:local.slim;
            String skin=local.nativeSkin!=null?local.nativeSkin:local.skin;
            if(skin!=null)avatar.skinTexture(platform.texture(skin),platform.textureId(skin));
        }
        if(avatar.cape==null&&local.replaceElytra)avatar.capeTexture(platform.texture(local.wingTexture),platform.textureId(local.wingTexture));
        if(avatar.cape==null&&local.cape!=null)avatar.capeTexture(platform.texture(local.cape),platform.textureId(local.cape));
        avatar.capeMotion=platform.capeMotion(java.util.UUID.fromString(local.id),local.capeMotion);avatar.elytra=local.elytra||Boolean.TRUE.equals(local.elytraEquipped);
        if(local.wingMatrices!=null&&local.partMatrices!=null) {
            // Preview the same native posture as the observed wing pose. Live
            // world rendering still needs an independent suppression receipt.
            avatar.partMatrices=local.partMatrices;avatar.wingMatrices=local.wingMatrices;
        }
        if(!avatar.elytra&&platform.recordedCapeMotionActive()&&avatar.capeMotion==null)avatar.cape=null;
        BufferedImage preview=AvatarRenderer.preview(avatar,(x1-x0)*guiScale,(y1-y0)*guiScale,scale*guiScale,
                (float)Math.atan((mx-(x0+x1)/2f)/100f)*0.6f,(float)Math.atan((my-(y0+y1)/2f)/100f)*0.25f);
        g.drawImage(preview,x0,y0,x1-x0,y1-y0,null);
    }
    @Override public void push() { stack.push(g); g = (Graphics2D) g.create(); }
    @Override public void pop() {
        if (stack.isEmpty()) throw new IllegalStateException("unbalanced graphics stack");
        g.dispose(); g = stack.pop();
    }
    @Override public void translate(float x, float y) { g.translate(x, y); }
    @Override public void rotate(float degrees) { g.rotate(Math.toRadians(degrees)); }
    @Override public void scale(float scale) { g.scale(scale, scale); }
    @Override public void scissor(int x0, int y0, int x1, int y1) {
        clip = g.getClip(); g.clipRect(x0, y0, Math.max(0, x1 - x0), Math.max(0, y1 - y0));
    }
    @Override public void endScissor() { g.setClip(clip); }
    @Override public void close() {
        while (!stack.isEmpty()) pop();
        g.dispose();
    }
}
