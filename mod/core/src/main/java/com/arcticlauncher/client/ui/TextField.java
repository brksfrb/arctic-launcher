package com.arcticlauncher.client.ui;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;

/**
 * A one-line text box with a cursor and a selection: type, move with the arrow keys (Ctrl: by
 * word; Shift: select), Home and End, click or drag to place the cursor or select, Backspace and
 * Delete (Ctrl: a word), Ctrl+A / C / X / V. Password boxes show dots and can't be copied.
 * Focus comes from its {@link Page}.
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
	/** Where typing goes, in chars of {@link #text}. */
	private int cursor;
	/** The other end of the selection; the same as the cursor when nothing is selected. */
	private int anchor;
	/** First char of the text that is drawn (the box scrolls to keep the cursor in view). */
	private int scrollStart;
	/** Pixel offset of each char boundary from the text's left edge, for the drawn part. */
	private int[] edges = new int[] {0};
	private Runnable onChange;
	private Runnable onEnter;

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
		cursor = text.length();
		anchor = cursor;
		scrollStart = 0;
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

	public String text() {
		return text;
	}

	public boolean isFocused() {
		return focused;
	}

	private boolean hasSelection() {
		return anchor != cursor;
	}

	private int selStart() {
		return Math.min(anchor, cursor);
	}

	private int selEnd() {
		return Math.max(anchor, cursor);
	}

	private String shown() {
		return password ? repeat(DOT, text.length()) : text;
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
		String all = shown();
		cursor = Math.max(0, Math.min(cursor, all.length()));
		anchor = Math.max(0, Math.min(anchor, all.length()));
		// Scroll so the cursor is in view.
		scrollStart = Math.max(0, Math.min(scrollStart, all.length()));
		if (cursor < scrollStart) {
			scrollStart = cursor;
		}
		while (scrollStart < cursor && g.textWidth(all.substring(scrollStart, cursor)) > room - 4) {
			scrollStart++;
		}
		// As much as fits from there.
		int end = scrollStart;
		while (end < all.length() && g.textWidth(all.substring(scrollStart, end + 1)) <= room - 2) {
			end++;
		}
		String visible = all.substring(scrollStart, end);
		edges = new int[visible.length() + 1];
		for (int i = 0; i <= visible.length(); i++) {
			edges[i] = g.textWidth(visible.substring(0, i));
		}
		if (focused && hasSelection()) {
			int from = Math.max(selStart(), scrollStart) - scrollStart;
			int to = Math.min(selEnd(), end) - scrollStart;
			if (to > from) {
				g.fill(x + 4 + edges[from], ty - 1, x + 4 + edges[to], ty + 9, Draw.alpha(s.accent, 0.45f));
			}
		}
		g.text(visible, x + 4, ty, s.text, false);
		if (focused && !hasSelection() && (System.currentTimeMillis() / BLINK_MS) % 2 == 0) {
			int at = Math.max(0, Math.min(cursor - scrollStart, visible.length()));
			int cx = x + 4 + edges[at];
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

	/** The char boundary nearest to a click at {@code mx}. */
	private int indexAt(double mx) {
		double rel = mx - (x + 4);
		int best = 0;
		for (int i = 0; i < edges.length; i++) {
			if (Math.abs(edges[i] - rel) <= Math.abs(edges[best] - rel)) {
				best = i;
			}
		}
		return Math.min(text.length(), scrollStart + best);
	}

	@Override
	public boolean click(double mx, double my, int button) {
		if (!enabled || button != Keys.MOUSE_LEFT) {
			return false;
		}
		cursor = indexAt(mx);
		if (!ArcticClient.platform().shiftDown()) {
			anchor = cursor;
		}
		return true;
	}

	@Override
	public boolean drag(double mx, double my, int button) {
		if (!enabled || button != Keys.MOUSE_LEFT) {
			return false;
		}
		cursor = indexAt(mx);
		return true;
	}

	/** A key while focused; true if the box used it. */
	boolean keyPressed(int key) {
		boolean ctrl = ArcticClient.platform().controlDown();
		boolean shift = ArcticClient.platform().shiftDown();
		switch (key) {
			case Keys.A:
				if (ctrl) {
					anchor = 0;
					cursor = text.length();
					return true;
				}
				return false;
			case Keys.C:
			case Keys.X:
				if (!ctrl) {
					return false;
				}
				if (!password && !text.isEmpty()) {
					// Nothing selected means the whole line, so an address can always be copied.
					String copied = hasSelection() ? text.substring(selStart(), selEnd()) : text;
					ArcticClient.platform().setClipboard(copied);
					if (key == Keys.X) {
						if (hasSelection()) {
							deleteSelection();
						} else {
							set("");
							cursor = 0;
							anchor = 0;
						}
					}
				}
				return true;
			case Keys.V:
				if (!ctrl) {
					return false;
				}
				String clip = ArcticClient.platform().clipboard();
				for (int i = 0; clip != null && i < clip.length(); ) {
					int c = clip.codePointAt(i);
					type(c);
					i += Character.charCount(c);
				}
				return true;
			case Keys.BACKSPACE:
				if (hasSelection()) {
					deleteSelection();
				} else if (cursor > 0) {
					int from = ctrl ? wordStart(text.substring(0, cursor)) : cursor - lastCharLength(text.substring(0, cursor));
					remove(from, cursor);
				}
				return true;
			case Keys.DELETE:
				if (hasSelection()) {
					deleteSelection();
				} else if (cursor < text.length()) {
					int to = ctrl ? wordEnd(text, cursor) : cursor + Character.charCount(text.codePointAt(cursor));
					remove(cursor, to);
				}
				return true;
			case Keys.LEFT:
				if (hasSelection() && !shift) {
					cursor = selStart();
					anchor = cursor;
				} else {
					int to = ctrl ? wordStart(text.substring(0, cursor)) : Math.max(0, cursor - (cursor > 0 ? lastCharLength(text.substring(0, cursor)) : 0));
					moveTo(to, shift);
				}
				return true;
			case Keys.RIGHT:
				if (hasSelection() && !shift) {
					cursor = selEnd();
					anchor = cursor;
				} else {
					int to = ctrl ? wordEnd(text, cursor)
							: cursor < text.length() ? cursor + Character.charCount(text.codePointAt(cursor)) : cursor;
					moveTo(to, shift);
				}
				return true;
			case Keys.HOME:
				moveTo(0, shift);
				return true;
			case Keys.END:
				moveTo(text.length(), shift);
				return true;
			default:
				return false;
		}
	}

	private void moveTo(int to, boolean extend) {
		cursor = Math.max(0, Math.min(text.length(), to));
		if (!extend) {
			anchor = cursor;
		}
	}

	/** A typed character while focused (replaces the selection). */
	void type(int codepoint) {
		if (!filter.accepts(codepoint)) {
			return;
		}
		int len = Character.charCount(codepoint);
		int kept = text.length() - (selEnd() - selStart());
		if (kept + len > maxLength) {
			return;
		}
		String left = text.substring(0, selStart());
		String right = text.substring(selEnd());
		int at = left.length() + len;
		set(left + new String(Character.toChars(codepoint)) + right);
		cursor = at;
		anchor = at;
	}

	/** Anything else ends a selection. */
	void unselect() {
		anchor = cursor;
	}

	private void deleteSelection() {
		remove(selStart(), selEnd());
	}

	private void remove(int from, int to) {
		from = Math.max(0, Math.min(from, text.length()));
		to = Math.max(from, Math.min(to, text.length()));
		set(text.substring(0, from) + text.substring(to));
		cursor = from;
		anchor = from;
	}

	private static int lastCharLength(String s) {
		return s.length() > 1 && Character.isLowSurrogate(s.charAt(s.length() - 1)) ? 2 : 1;
	}

	/** Where the word before the end of {@code s} starts: spaces before it go too. */
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

	/** Where the word at {@code from} ends: the spaces after it go too. */
	static int wordEnd(String s, int from) {
		int i = from;
		while (i < s.length() && Character.isWhitespace(s.charAt(i))) {
			i++;
		}
		while (i < s.length() && !Character.isWhitespace(s.charAt(i))) {
			i++;
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
