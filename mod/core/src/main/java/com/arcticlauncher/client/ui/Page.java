package com.arcticlauncher.client.ui;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Backdrop;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import java.util.ArrayList;
import java.util.List;

/**
 * A full screen made of {@link Widget}s. Adapters show it inside a vanilla
 * screen and forward input; everything else happens here.
 */
public abstract class Page {
	private final List<Widget> widgets = new ArrayList<Widget>();
	protected int width;
	protected int height;
	private Widget pressed;

	/** Lay out for a screen size (also on resize). */
	public final void init(int width, int height) {
		this.width = width;
		this.height = height;
		widgets.clear();
		build();
	}

	protected abstract void build();

	protected final void rebuild() {
		init(width, height);
	}

	protected final <T extends Widget> T add(T widget) {
		widgets.add(widget);
		return widget;
	}

	protected static Style style() {
		return ArcticClient.style();
	}

	/** Draw the Arctic backdrop; otherwise the game shows behind (blurred). */
	public boolean ownBackground() {
		return !ArcticClient.platform().inWorld();
	}

	/** Without {@link #ownBackground()}: blur and dim the game behind. */
	public boolean dimWorld() {
		return true;
	}

	public boolean pausesGame() {
		return true;
	}

	public boolean closeOnEscape() {
		return true;
	}

	public void render(Gfx g, int mx, int my) {
		Style s = style();
		if (ownBackground()) {
			Backdrop.render(g, s);
		}
		drawBehind(g, s, mx, my);
		for (Widget w : snapshot()) {
			w.render(g, s, mx, my);
		}
		drawAbove(g, s, mx, my);
	}

	protected void drawBehind(Gfx g, Style s, int mx, int my) {}

	protected void drawAbove(Gfx g, Style s, int mx, int my) {}

	private Widget[] snapshot() {
		return widgets.toArray(new Widget[0]);
	}

	public boolean mouseClicked(double mx, double my, int button) {
		Widget[] all = snapshot();
		for (int i = all.length - 1; i >= 0; i--) {
			Widget w = all[i];
			if (w.enabled && w.contains(mx, my)) {
				pressed = w;
				return w.click(mx, my, button);
			}
		}
		return false;
	}

	public boolean mouseReleased(double mx, double my, int button) {
		Widget w = pressed;
		pressed = null;
		if (w == null) {
			return false;
		}
		w.release(mx, my, button);
		return true;
	}

	public boolean mouseDragged(double mx, double my, int button) {
		return pressed != null && pressed.drag(mx, my, button);
	}

	public boolean mouseScrolled(double mx, double my, double amount) {
		return false;
	}

	/**
	 * A key press: {@code key} in core codes ({@link Keys}), {@code nativeKey}
	 * as the game reported it (for binding keys, see {@code Platform.keyName}).
	 */
	public boolean keyPressed(int key, int nativeKey) {
		if (key == Keys.ESCAPE && closeOnEscape()) {
			close();
			return true;
		}
		return false;
	}

	public void close() {
		ArcticClient.platform().closePage();
	}

	/** Called 20 times a second while shown. */
	public void tick() {}
}
