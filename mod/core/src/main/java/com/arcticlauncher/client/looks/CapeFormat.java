package com.arcticlauncher.client.looks;

import java.awt.Graphics2D;
import java.awt.RenderingHints;
import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import javax.imageio.ImageIO;

/**
 * Elytra art for capes that have none. Minecraft draws an elytra from the right part of the cape
 * image (the 64×32 layout's wing box at 22,0); a cape painted only where the cape itself goes
 * leaves that empty, and since the game hides the cape while an elytra is worn, both vanish. Such
 * a cape gets wings painted from its own art: its front on the wings' outside (the face at x 36,
 * the one vanilla's elytra paints), its back on the inside against the player. Checked per frame.
 */
public final class CapeFormat {
	/** The cape layout is 64 wide (and 32 tall per frame); bigger images scale it. */
	private static final int BASE = 64;
	/** The cape's front and back faces (x, y, w, h) and the wing's outer and inner faces. */
	private static final int[] CAPE_FRONT = {1, 1, 10, 16};
	private static final int[] CAPE_BACK = {12, 1, 10, 16};
	private static final int[] WING_OUTSIDE = {36, 2, 10, 20};
	private static final int[] WING_INSIDE = {24, 2, 10, 20};
	/**
	 * The wing's thin faces (it's 10 wide, 20 tall, 2 thick): its sides, top and bottom. Left
	 * empty, the wings are see-through edge-on (from the side, or from above between them).
	 */
	private static final int[] WING_SIDE_IN = {22, 2, 2, 20};
	private static final int[] WING_SIDE_OUT = {34, 2, 2, 20};
	private static final int[] WING_TOP = {24, 0, 10, 2};
	private static final int[] WING_BOTTOM = {34, 0, 10, 2};
	/** The edge columns and rows of the wing's inner face, which those are painted from. */
	private static final int[] INSIDE_LEFT = {24, 2, 1, 20};
	private static final int[] INSIDE_RIGHT = {33, 2, 1, 20};
	private static final int[] INSIDE_TOP = {24, 2, 10, 1};
	private static final int[] INSIDE_BOTTOM = {24, 21, 10, 1};
	/** All four thin faces, to tell whether a frame has them. */
	private static final int[][] WING_EDGES = {WING_SIDE_IN, WING_SIDE_OUT, WING_TOP, WING_BOTTOM};
	/** The whole wing box's area: empty here means that frame has no elytra art. */
	private static final int[] WING_AREA = {22, 0, 24, 22};

	private CapeFormat() {}

	/** The cape (as PNG) with elytra art; the input itself when it has some or isn't a cape. */
	public static byte[] withElytra(byte[] png) {
		try {
			BufferedImage in = ImageIO.read(new ByteArrayInputStream(png));
			if (in == null) {
				return png;
			}
			BufferedImage out = withElytra(in);
			if (out == null) {
				return png;
			}
			ByteArrayOutputStream bytes = new ByteArrayOutputStream();
			ImageIO.write(out, "png", bytes);
			return bytes.toByteArray();
		} catch (Exception e) {
			return png;
		}
	}

	/** Null when nothing needs painting (or the image isn't a cape strip). */
	static BufferedImage withElytra(BufferedImage in) {
		int w = in.getWidth();
		int frames = Looks.capeFrames(w, in.getHeight());
		if (w % BASE != 0 || frames == 0) {
			return null;
		}
		int s = w / BASE;
		int frameHeight = w / 2;
		boolean any = false;
		for (int f = 0; f < frames && !any; f++) {
			int top = f * frameHeight;
			any = empty(in, WING_AREA, top, s) || edgesEmpty(in, top, s);
		}
		if (!any) {
			return null;
		}
		BufferedImage out = new BufferedImage(w, in.getHeight(), BufferedImage.TYPE_INT_ARGB);
		out.setRGB(0, 0, w, in.getHeight(), in.getRGB(0, 0, w, in.getHeight(), null, 0, w), 0, w);
		Graphics2D g = out.createGraphics();
		g.setRenderingHint(RenderingHints.KEY_INTERPOLATION, RenderingHints.VALUE_INTERPOLATION_NEAREST_NEIGHBOR);
		for (int f = 0; f < frames; f++) {
			int top = f * frameHeight;
			if (empty(in, WING_AREA, top, s)) {
				copy(g, in, CAPE_FRONT, WING_OUTSIDE, s, top);
				copy(g, in, CAPE_BACK, WING_INSIDE, s, top);
			}
		}
		// The wings' thin faces from their inner face's border (painted just now, or the cape's own
		// elytra art, which often leaves them out).
		for (int f = 0; f < frames; f++) {
			int top = f * frameHeight;
			if (edgesEmpty(out, top, s)) {
				copy(g, out, INSIDE_LEFT, WING_SIDE_IN, s, top);
				copy(g, out, INSIDE_RIGHT, WING_SIDE_OUT, s, top);
				copy(g, out, INSIDE_TOP, WING_TOP, s, top);
				copy(g, out, INSIDE_BOTTOM, WING_BOTTOM, s, top);
			}
		}
		g.dispose();
		return out;
	}

	/** Whether all four thin faces of the frame at {@code top} are empty. */
	private static boolean edgesEmpty(BufferedImage img, int top, int s) {
		for (int[] edge : WING_EDGES) {
			if (!empty(img, edge, top, s)) {
				return false;
			}
		}
		return true;
	}

	/** Whether the area {@code a} (x, y, w, h) of the frame at {@code top} has no visible pixel. */
	private static boolean empty(BufferedImage img, int[] a, int top, int s) {
		for (int y = a[1] * s; y < (a[1] + a[3]) * s; y++) {
			for (int x = a[0] * s; x < (a[0] + a[2]) * s; x++) {
				if ((img.getRGB(x, top + y) >>> 24) != 0) {
					return false;
				}
			}
		}
		return true;
	}

	/** The face {@code from} of the frame at {@code top}, stretched onto the face {@code to}. */
	private static void copy(Graphics2D g, BufferedImage in, int[] from, int[] to, int s, int top) {
		g.drawImage(in,
				to[0] * s, top + to[1] * s, (to[0] + to[2]) * s, top + (to[1] + to[3]) * s,
				from[0] * s, top + from[1] * s, (from[0] + from[2]) * s, top + (from[1] + from[3]) * s,
				null);
	}
}
