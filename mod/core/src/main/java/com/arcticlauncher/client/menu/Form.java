package com.arcticlauncher.client.menu;

import java.util.function.BooleanSupplier;
import java.util.function.Consumer;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.ui.KeyButton;
import com.arcticlauncher.client.ui.Label;
import com.arcticlauncher.client.ui.Scroll;
import com.arcticlauncher.client.ui.Toggle;
import com.arcticlauncher.client.ui.Widget;

/**
 * A settings tab laid out top to bottom in a scrolling box: sections,
 * switches (some with a key), key rows, notes and custom rows. Every change
 * is saved right away.
 */
final class Form {
	static final int ROW = 24;
	static final int KEY_W = 76;
	private static final int SECTION_H = 16;
	private static final int SECTION_GAP = 8;
	private static final int NOTE_H = 11;
	private static final int SCROLLBAR = 6;

	private final Host host;
	private final Scroll scroll;
	final int x;
	final int w;
	private int y;
	private boolean first = true;

	Form(Host host, int x, int top, int w, int bottom) {
		this.host = host;
		this.x = x;
		// Room on the right for the scroll bar.
		this.w = w - SCROLLBAR;
		this.y = top;
		this.scroll = host.scroll(x, top, x + w - SCROLLBAR + 2, bottom);
	}

	Scroll scroll() {
		return scroll;
	}

	/** Where the next row starts (unscrolled). */
	int y() {
		return y;
	}

	/** Place a widget (unscrolled coordinates). */
	<T extends Widget> T put(T widget, int wx, int wy, int ww, int wh) {
		widget.bounds(wx, wy, ww, wh);
		host.add(widget);
		return scroll.add(widget);
	}

	/** Take a row of {@code h} pixels; returns its top. */
	int row(int h) {
		int top = y;
		y += h;
		first = false;
		return top;
	}

	void gap(int h) {
		y += h;
	}

	void section(String title) {
		if (!first) {
			y += SECTION_GAP;
		}
		put(new Label(title, Label.Kind.SECTION), x, y, w, SECTION_H);
		y += SECTION_H + 4;
		first = false;
	}

	/** A muted line of explanation. */
	void note(String text) {
		put(new Label(text, Label.Kind.MUTED), x + 4, row(NOTE_H), w - 4, NOTE_H);
	}

	Toggle toggle(String name, String hint, BooleanSupplier get, Consumer<Boolean> set) {
		return put(new Toggle(name, hint, binding(get, set)), x, row(ROW), w, ROW);
	}

	/** A switch with the key that also switches it (or that you hold). */
	void toggleKey(String name, String hint, BooleanSupplier get, Consumer<Boolean> set, KeyButton.Binding key) {
		int top = row(ROW);
		put(new Toggle(name, hint, binding(get, set)), x, top, w - KEY_W - 6, ROW);
		keyButton(key, top);
	}

	/** Just a key: its name and what it does, and the key button. */
	void key(String name, String hint, KeyButton.Binding key) {
		int top = row(ROW);
		put(new Label(name, Label.Kind.TEXT), x + 4, top + 3, w - KEY_W - 10, 9);
		put(new Label(hint, Label.Kind.MUTED), x + 4, top + 13, w - KEY_W - 10, 9);
		keyButton(key, top);
	}

	private void keyButton(final KeyButton.Binding key, int top) {
		KeyButton button = put(new KeyButton(new KeyButton.Binding() {
			@Override
			public String get() {
				return key.get();
			}

			@Override
			public void set(String k) {
				key.set(k);
				ArcticClient.saveConfig();
			}
		}), x + w - KEY_W, top + 3, KEY_W, ROW - 6);
		host.listenKeys(button);
	}

	private static final int LABEL_W = 96;
	private static final int SWATCH = 12;
	private static final int SWATCH_GAP = 5;
	private static final int CHOICE_H = 16;
	private static final int STEP_BTN = 16;
	private static final int STEP_VALUE_W = 36;

	private void rowLabel(int top, String text) {
		put(new Label(text, Label.Kind.TEXT), x + 4, top, LABEL_W - 4, ROW);
	}

	/** "Label   ■ ■ ■ ■": pick one color; the chosen one gets a ring. */
	void swatches(String label, int[] colors, final java.util.function.IntSupplier get, final java.util.function.IntConsumer set) {
		int top = row(ROW);
		rowLabel(top, label);
		int sx = x + LABEL_W;
		for (final int color : colors) {
			put(new Swatch(color, get.getAsInt() == color, () -> {
				set.accept(color);
				ArcticClient.saveConfig();
				host.rebuild();
			}), sx, top + (ROW - SWATCH) / 2, SWATCH, SWATCH);
			sx += SWATCH + SWATCH_GAP;
		}
	}

	/** "Label   [One] [Two] [Three]": pick one option. */
	void choice(String label, String[] options, final java.util.function.IntSupplier get, final java.util.function.IntConsumer set) {
		int top = row(ROW);
		rowLabel(top, label);
		int bw = (w - LABEL_W) / options.length;
		for (int i = 0; i < options.length; i++) {
			final int index = i;
			put(new com.arcticlauncher.client.ui.Button(options[i], () -> {
				set.accept(index);
				ArcticClient.saveConfig();
				host.rebuild();
			}).selected(get.getAsInt() == i), x + LABEL_W + i * bw, top + (ROW - CHOICE_H) / 2, bw - 2, CHOICE_H);
		}
	}

	/** "Label        -  1.2  +": a number in steps, shown with {@code format}. */
	void stepper(String label, final java.util.function.DoubleSupplier get, final java.util.function.DoubleConsumer set,
			final double min, final double max, final double step, final String format) {
		int top = row(ROW);
		rowLabel(top, label);
		int right = x + w;
		int by = top + (ROW - STEP_BTN) / 2;
		final Label value = new Label(String.format(java.util.Locale.ROOT, format, get.getAsDouble()), Label.Kind.TEXT);
		put(new com.arcticlauncher.client.ui.Button("-", () -> {
			set.accept(Math.max(min, Math.round((get.getAsDouble() - step) / step) * step));
			value.text(String.format(java.util.Locale.ROOT, format, get.getAsDouble()));
			ArcticClient.saveConfig();
		}), right - STEP_BTN * 2 - STEP_VALUE_W, by, STEP_BTN, STEP_BTN);
		put(value, right - STEP_BTN - STEP_VALUE_W + 8, top, STEP_VALUE_W - 8, ROW);
		put(new com.arcticlauncher.client.ui.Button("+", () -> {
			set.accept(Math.min(max, Math.round((get.getAsDouble() + step) / step) * step));
			value.text(String.format(java.util.Locale.ROOT, format, get.getAsDouble()));
			ArcticClient.saveConfig();
		}), right - STEP_BTN, by, STEP_BTN, STEP_BTN);
	}

	/** Call last: sets how far the box scrolls. */
	void done() {
		scroll.extendTo(y);
		scroll.settle();
	}

	/** A key binding on a config field. */
	static KeyButton.Binding key(final java.util.function.Supplier<String> get, final java.util.function.Consumer<String> set) {
		return new KeyButton.Binding() {
			@Override
			public String get() {
				return get.get();
			}

			@Override
			public void set(String k) {
				set.accept(k);
			}
		};
	}

	static Toggle.Binding binding(final BooleanSupplier get, final Consumer<Boolean> set) {
		return new Toggle.Binding() {
			@Override
			public boolean get() {
				return get.getAsBoolean();
			}

			@Override
			public void set(boolean on) {
				set.accept(on);
				ArcticClient.saveConfig();
			}
		};
	}
}
