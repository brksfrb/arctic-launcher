package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Widget;
import java.util.function.DoubleConsumer;
import java.util.function.DoubleSupplier;

/** A number on a track: click or drag to set it (in steps), let go to keep it. */
final class SliderBar extends Widget {
	private static final int KNOB_W = 6;
	private static final int KNOB_H = 12;
	private static final int TRACK_H = 4;

	private final DoubleSupplier get;
	private final DoubleConsumer set;
	private final double min;
	private final double max;
	private final double step;
	/** Runs after every change while dragging (the shown value follows). */
	private final Runnable changed;
	/** Runs when the mouse is let go (save it). */
	private final Runnable finished;
	private boolean dragging;

	SliderBar(DoubleSupplier get, DoubleConsumer set, double min, double max, double step, Runnable changed, Runnable finished) {
		this.get = get;
		this.set = set;
		this.min = min;
		this.max = max;
		this.step = step;
		this.changed = changed;
		this.finished = finished;
	}

	@Override
	protected void draw(Gfx g, Style s, int mx, int my, float dt) {
		int trackTop = y + (h - TRACK_H) / 2;
		double share = max > min ? (Math.max(min, Math.min(max, get.getAsDouble())) - min) / (max - min) : 0;
		int knobX = x + KNOB_W / 2 + (int) Math.round(share * (w - KNOB_W));
		Draw.round(g, x, trackTop, x + w, trackTop + TRACK_H, 2, s.border);
		Draw.round(g, x, trackTop, knobX, trackTop + TRACK_H, 2, s.accent);
		int ring = dragging || hover > 0.5f ? s.text : s.muted;
		Draw.round(g, knobX - KNOB_W / 2, y + (h - KNOB_H) / 2, knobX + KNOB_W / 2, y + (h + KNOB_H) / 2, 2, ring);
	}

	@Override
	public boolean click(double mx, double my, int button) {
		if (button != Keys.MOUSE_LEFT) {
			return false;
		}
		dragging = true;
		apply(mx);
		return true;
	}

	@Override
	public boolean drag(double mx, double my, int button) {
		if (!dragging || button != Keys.MOUSE_LEFT) {
			return false;
		}
		apply(mx);
		return true;
	}

	@Override
	public void release(double mx, double my, int button) {
		if (dragging) {
			dragging = false;
			finished.run();
		}
	}

	private void apply(double mx) {
		double share = Math.max(0, Math.min(1, (mx - x - KNOB_W / 2.0) / Math.max(1, w - KNOB_W)));
		double value = min + share * (max - min);
		value = Math.round(value / step) * step;
		set.accept(Math.max(min, Math.min(max, value)));
		changed.run();
	}
}
