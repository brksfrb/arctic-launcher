package com.arcticlauncher.client.ui;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;

/**
 * A one-line text box: type, Backspace (Ctrl+Backspace: a word), Ctrl+V to
 * paste, Ctrl+A to select everything (the next key replaces it). Password
 * boxes show dots. Focus comes from its {@link Page}.
 */
public class TextField extends Widget {
	private static final long BLINK_MS = 500;
	private static final char DOT = '•';

	/** Which characters the box accepts. */
	public interface Filter {
		boolean accepts(int codepoint);
	}

	/** Anything printable. */
	public static final Filter ANY = new Filter() {
		@Override
		public boolean accepts(int c) {
			return c >= ' ' && c != 0x7F && c != 0xA7 && !Character.isISOControl(c);
		}
	};

	/** Digits only. */
	public static final Filter DIGITS = new Filter() {
		@Override
		public boolean accepts(int c) {
			return c >= '0' && c <= '9';
		}
	};

	private final String hint;
	private final int maxLength;
	private final Filter filter;
	private boolean password;
	private String text = "";
	boolean focused;
	/** Ctrl+A: everything is selected, and typing replaces it. */
	private boolean allSelected;
	private Runnable onChange;

	public TextField(String hint, int maxLength, Filter filter) {
		this.hint = hint;
		this.maxLength = maxLength;
		this.filter = filter;
	}

	public TextField password() {
		password = true;
		return this;
	}

	public TextField text(String value) {
		text = value == null ? "" : value;
		if (text.length() > maxLength) {
			text = text.substring(0, maxLength);
		}
		return this;
	}

	public TextField onChange(Runnable r) {
		onChange = r;
		return this;
	}

	/** Enter runs this (and keeps the field focused) instead of leaving it. */
	public TextField onEnter(Runnable r) {
		onEnter = r;
		return this;
	}

	/** Enter was pressed: true if the field handled it. */
	boolean submit() {
		if (onEnter == null) {
			return false;
		}
		onEnter.run();
		return true;
	}

	private Runnable onEnter;

	public String text() {
		return text;
	}

	public boolean isFocused() {
		return focused;
	}

	@Override
	protected void draw(Gfx g, Style s, int mx, int my, float dt) {
		Skin.field(g, s, x, y, w, h, focused);
		int room = w - 8;
		int ty = y + (h - 8) / 2;
		if (text.isEmpty() && !focused) {
			g.text(Draw.fit(g, hint, room), x + 4, ty, s.muted, false);
			return;
		}
		String shown = password ? repeat(DOT, text.length()) : text;
		// Keep the end (where typing happens) in view.
		while (shown.length() > 0 && g.textWidth(shown) > room - 4) {
			shown = shown.substring(1);
		}
		if (focused && allSelected && !shown.isEmpty()) {
			g.fill(x + 3, ty - 1, x + 5 + g.textWidth(shown), ty + 9, Draw.alpha(s.accent, 0.45f));
		}
		g.text(shown, x + 4, ty, s.text, false);
		if (focused && !allSelected && (System.currentTimeMillis() / BLINK_MS) % 2 == 0) {
			int cx = x + 4 + g.textWidth(shown) + 1;
			g.fill(cx, ty - 1, cx + 1, ty + 9, s.text);
		}
	}

	private static String repeat(char c, int n) {
		StringBuilder b = new StringBuilder(n);
		for (int i = 0; i < n; i++) {
			b.append(c);
		}
		return b.toString();
	}

	@Override
	public boolean click(double mx, double my, int button) {
		return enabled && button == Keys.MOUSE_LEFT;
	}

	/** A key while focused; true if the box used it. */
	boolean keyPressed(int key) {
		boolean ctrl = ArcticClient.platform().controlDown();
		if (key == Keys.A && ctrl) {
			allSelected = !text.isEmpty();
			return true;
		}
		if (key == Keys.BACKSPACE) {
			if (allSelected) {
				allSelected = false;
				set("");
			} else if (!text.isEmpty()) {
				set(text.substring(0, ctrl ? wordStart(text) : text.length() - lastCharLength(text)));
			}
			return true;
		}
		if (key == Keys.V && ctrl) {
			replaceSelection();
			String clip = ArcticClient.platform().clipboard();
			for (int i = 0; clip != null && i < clip.length(); ) {
				int c = clip.codePointAt(i);
				type(c);
				i += Character.charCount(c);
			}
			return true;
		}
		return false;
	}

	/** A typed character while focused. */
	void type(int codepoint) {
		if (!filter.accepts(codepoint)) {
			return;
		}
		replaceSelection();
		if (text.length() + Character.charCount(codepoint) > maxLength) {
			return;
		}
		set(text + new String(Character.toChars(codepoint)));
	}

	/** Anything else ends a selection. */
	void unselect() {
		allSelected = false;
	}

	private void replaceSelection() {
		if (allSelected) {
			allSelected = false;
			set("");
		}
	}

	private static int lastCharLength(String s) {
		return Character.isLowSurrogate(s.charAt(s.length() - 1)) && s.length() > 1 ? 2 : 1;
	}

	/** Where the last word starts (Ctrl+Backspace): spaces before it go too. */
	static int wordStart(String s) {
		int i = s.length();
		while (i > 0 && Character.isWhitespace(s.charAt(i - 1))) {
			i--;
		}
		while (i > 0 && !Character.isWhitespace(s.charAt(i - 1))) {
			i--;
		}
		return i;
	}

	private void set(String value) {
		text = value;
		if (onChange != null) {
			onChange.run();
		}
	}
}
