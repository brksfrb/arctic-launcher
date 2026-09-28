package com.arcticlauncher.client.ui;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Backdrop;
import com.arcticlauncher.client.gfx.Draw;
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
	/** When the page was first drawn: it eases in for {@link #OPEN_SECONDS}. */
	private long openedAt;
	private static final float OPEN_SECONDS = 0.18f;
	/** It rises this far (GUI pixels) and grows from {@link #OPEN_SCALE} to full size. */
	private static final float OPEN_RISE = 8f;
	private static final float OPEN_SCALE = 0.97f;
	private Widget pressed;
	/** The text box being typed in, if any. */
	private TextField focused;

	/** Lay out for a screen size (also on resize). */
	public final void init(int width, int height) {
		this.width = width;
		this.height = height;
		// A text box the page adds again keeps the keyboard (live search).
		TextField keep = focused;
		focus(null);
		widgets.clear();
		build();
		if (keep != null && widgets.contains(keep)) {
			focus(keep);
		}
	}

	/** Move typing to {@code field} (null = nowhere). */
	protected final void focus(TextField field) {
		if (focused == field) {
			return;
		}
		if (focused != null) {
			focused.focused = false;
			focused.unselect();
			ArcticClient.platform().textInput(focused, false);
		}
		focused = field;
		if (field != null) {
			field.focused = true;
			ArcticClient.platform().textInput(field, true);
		}
	}

	/** A typed character (Unicode code point). */
	public boolean charTyped(int codepoint) {
		if (focused == null) {
			return false;
		}
		focused.type(codepoint);
		return true;
	}

	/** The page is going away: stop typing. */
	public void removed() {
		focus(null);
	}

	/** Tab: the next text box on the page. */
	private void focusNext() {
		List<TextField> fields = new ArrayList<TextField>();
		for (Widget w : widgets) {
			if (w instanceof TextField && w.enabled && w.visible) {
				fields.add((TextField) w);
			}
		}
		if (!fields.isEmpty()) {
			int at = fields.indexOf(focused);
			focus(fields.get((at + 1) % fields.size()));
		}
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

	/** Where Escape goes instead of closing (an inner page's outer one), or null. */
	protected Page escapeTo() {
		return null;
	}

	public boolean closeOnEscape() {
		return true;
	}

	public void render(Gfx g, int mx, int my) {
		Draw.fancy = ArcticClient.config().fancy;
		Hints.frame(mx, my);
		Style s = style();
		if (ownBackground()) {
			Backdrop.render(g, s);
		}
		float open = openProgress();
		boolean opening = open < 1f;
		if (opening) {
			// Ease out: quick at first, settling into place.
			float e = 1f - (1f - open) * (1f - open) * (1f - open);
			float cx = width / 2f;
			float cy = height / 2f;
			g.push();
			g.translate(cx, cy + (1f - e) * OPEN_RISE);
			g.scale(OPEN_SCALE + (1f - OPEN_SCALE) * e);
			g.translate(-cx, -cy);
		}
		drawBehind(g, s, mx, my);
		for (Widget w : snapshot()) {
			if (!w.visible || !w.onScreen()) {
				continue;
			}
			if (w.clip != null) {
				g.scissor(w.clip[0], w.clip[1], w.clip[2], w.clip[3]);
				w.render(g, s, mx, my);
				g.endScissor();
			} else {
				w.render(g, s, mx, my);
			}
		}
		drawAbove(g, s, mx, my);
		if (opening) {
			g.pop();
		}
		com.arcticlauncher.client.notice.Notices.render(g, s);
		Hints.draw(g, s, width, height);
	}

	/** Whether the page eases in when opened (not where things must stay put, like the HUD). */
	protected boolean animatesIn() {
		return true;
	}

	/** 0 when first drawn, 1 once the opening animation is done. */
	private float openProgress() {
		if (!animatesIn()) {
			return 1f;
		}
		long now = System.nanoTime();
		if (openedAt == 0L) {
			openedAt = now;
		}
		return Math.min(1f, (now - openedAt) / 1e9f / OPEN_SECONDS);
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
			if (w.enabled && w.visible && w.contains(mx, my)) {
				pressed = w;
				focus(w instanceof TextField ? (TextField) w : null);
				return w.click(mx, my, button);
			}
		}
		focus(null);
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
		if (focused != null) {
			if (key == Keys.ENTER && focused.submit()) {
				return true;
			}
			if (key == Keys.ESCAPE || key == Keys.ENTER) {
				focus(null);
				return true;
			}
			if (key == Keys.TAB) {
				focusNext();
				return true;
			}
			// Typing (Right Shift is just Shift here).
			return focused.keyPressed(key) || key == Keys.RIGHT_SHIFT;
		}
		if (key == Keys.TAB) {
			focusNext();
			return true;
		}
		if (key == Keys.ESCAPE) {
			Page outer = escapeTo();
			if (outer != null) {
				ArcticClient.platform().openPage(outer);
				return true;
			}
			if (closeOnEscape()) {
				close();
				return true;
			}
		}
		if (key == Keys.RIGHT_SHIFT) {
			return rightShift();
		}
		return false;
	}

	/** A text field has the keyboard (don't rebuild under the player's typing). */
	protected boolean typing() {
		return focused != null;
	}

	/** Right Shift: opens the Arctic menu over this page. */
	protected boolean rightShift() {
		ArcticClient.platform().openPage(ArcticClient.arcticMenu());
		return true;
	}

	public void close() {
		ArcticClient.platform().closePage();
	}

	/** Called 20 times a second while shown. */
	public void tick() {}
}
