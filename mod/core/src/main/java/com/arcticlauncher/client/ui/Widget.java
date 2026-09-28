package com.arcticlauncher.client.ui;

import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

/** A rectangle on a {@link Page} that draws itself and takes clicks. */
public abstract class Widget {
	private static final float HOVER_SPEED = 10f;

	public int x;
	public int y;
	public int w;
	public int h;
	public boolean enabled = true;
	/** Hidden widgets are neither drawn nor clicked. */
	public boolean visible = true;
	/** When set ({x0, y0, x1, y1}), drawn and clicked only inside it (a scrolling list). */
	public int[] clip;
	/** Hover animation, 0..1. */
	protected float hover;
	private long lastFrame;

	public Widget bounds(int x, int y, int w, int h) {
		this.x = x;
		this.y = y;
		this.w = w;
		this.h = h;
		return this;
	}

	public boolean contains(double mx, double my) {
		return mx >= x && my >= y && mx < x + w && my < y + h && inClip(mx, my);
	}

	/** Inside the clip box (or no clip); also false when scrolled fully out of it. */
	protected final boolean inClip(double mx, double my) {
		return clip == null || (mx >= clip[0] && my >= clip[1] && mx < clip[2] && my < clip[3]);
	}

	/** Some of it shows inside the clip box. */
	public boolean onScreen() {
		return clip == null || (y + h > clip[1] && y < clip[3]);
	}

	public final void render(Gfx g, Style s, int mx, int my) {
		long now = System.nanoTime();
		float dt = lastFrame == 0 ? 0f : Math.min(0.1f, (now - lastFrame) / 1e9f);
		lastFrame = now;
		boolean over = enabled && contains(mx, my);
		hover = approach(hover, over ? 1f : 0f, dt * HOVER_SPEED);
		draw(g, s, mx, my, dt);
	}

	/** Move {@code value} toward {@code target} by at most {@code step}. */
	protected static float approach(float value, float target, float step) {
		if (value < target) {
			return Math.min(target, value + step);
		}
		return Math.max(target, value - step);
	}

	protected abstract void draw(Gfx g, Style s, int mx, int my, float dt);

	/** Left or right click inside this widget. */
	public boolean click(double mx, double my, int button) {
		return false;
	}

	public void release(double mx, double my, int button) {}

	public boolean drag(double mx, double my, int button) {
		return false;
	}
}
