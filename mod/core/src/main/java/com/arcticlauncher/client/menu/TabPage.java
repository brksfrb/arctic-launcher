package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.KeyButton;
import com.arcticlauncher.client.ui.Page;
import com.arcticlauncher.client.ui.Scroll;
import com.arcticlauncher.client.ui.Toggle;
import com.arcticlauncher.client.ui.Widget;

/**
 * One screen of the Arctic menu in the shared panel: a header with a back
 * arrow, the icon and title (and the mod's switch), and the tab's content.
 */
final class TabPage extends Page {
	static final int MAX_W = 520;
	static final int MAX_H = 320;
	static final int MARGIN = 12;
	static final int PAD = 12;
	static final int HEADER = 40;
	private static final int ICON = 16;
	private static final int BACK = 18;
	private static final int SWITCH_W = 44;

	private final MenuTab tab;
	/** The mod this page belongs to (its switch shows in the header), or null. */
	private final Mod mod;
	/** Where the back arrow goes (the screen this was opened from); null = Mods. */
	private final Page back;
	private final java.util.List<KeyButton> keyButtons = new java.util.ArrayList<KeyButton>();
	private Scroll scroll;
	/** Where the tab's scroll box was, kept across rebuilds. */
	private int scrollOffset;
	private String shownState;
	int px;
	int py;
	int pw;
	int ph;

	TabPage(MenuTab tab) {
		this(tab, null, null);
	}

	TabPage(MenuTab tab, Mod mod, Page back) {
		this.tab = tab;
		this.mod = mod;
		this.back = back;
	}

	@Override
	public boolean pausesGame() {
		return false;
	}

	@Override
	protected void build() {
		pw = Math.min(MAX_W, width - MARGIN * 2);
		ph = Math.min(MAX_H, height - MARGIN * 2);
		px = (width - pw) / 2;
		py = (height - ph) / 2;
		keyButtons.clear();
		if (scroll != null) {
			scrollOffset = scroll.offset();
		}
		scroll = null;
		add(new Button("<", () -> ArcticClient.platform().openPage(back != null ? back : new ModsScreen())))
				.bounds(px + PAD, py + (HEADER - BACK) / 2, BACK, BACK);
		if (mod != null && mod.hasSwitch()) {
			add(new Toggle("", null, Form.binding(mod::on, mod::setOn)))
					.bounds(px + pw - PAD - SWITCH_W, py + (HEADER - 16) / 2, SWITCH_W, 16);
		}
		shownState = tab.state();
		tab.build(new Host() {
			@Override
			public Widget add(Widget w) {
				return TabPage.this.add(w);
			}

			@Override
			public void rebuild() {
				TabPage.this.rebuild();
			}

			@Override
			public void listenKeys(KeyButton button) {
				keyButtons.add(button);
			}

			@Override
			public Scroll scroll(int x0, int y0, int x1, int y1) {
				scroll = new Scroll(x0, y0, x1, y1, scrollOffset);
				return scroll;
			}
		}, contentX(), contentTop(), contentW(), py + ph - PAD);
	}

	int contentX() {
		return px + PAD;
	}

	int contentW() {
		return pw - PAD * 2;
	}

	int contentTop() {
		return py + HEADER + 4;
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		Skin.panel(g, s, px, py, px + pw, py + ph);
		int tx = px + PAD + BACK + 8;
		String icon = mod != null ? mod.iconKey() : null;
		if (icon != null) {
			g.texture(icon, tx, py + (HEADER - ICON) / 2, ICON, ICON, 0, 0, 32, 32, 32, 32);
			tx += ICON + 6;
		}
		int room = pw - (tx - px) - PAD - (mod != null && mod.hasSwitch() ? SWITCH_W + 8 : 0);
		g.text("§l" + Draw.fit(g, tab.title(), room), tx, py + 10, s.text, false);
		String status = tab.status();
		String hint = status != null ? status : tab.hint();
		g.text(Draw.fit(g, hint, room), tx, py + 22, status != null ? s.accent : s.muted, false);
		g.fill(px + PAD, py + HEADER, px + pw - PAD, py + HEADER + 1, s.border);
		tab.draw(g, s, mx, my);
	}

	@Override
	protected void drawAbove(Gfx g, Style s, int mx, int my) {
		if (scroll != null) {
			scroll.drawBar(g, s);
		}
		tab.drawAbove(g, s, mx, my);
	}

	@Override
	public boolean mouseScrolled(double mx, double my, double amount) {
		return scroll != null && scroll.wheel(mx, my, amount);
	}

	/** Escape goes back out, like the arrow. */
	@Override
	protected Page escapeTo() {
		return back != null ? back : new ModsScreen();
	}

	/** A key button that's listening takes the key first. */
	@Override
	public boolean keyPressed(int key, int nativeKey) {
		for (KeyButton b : keyButtons) {
			if (b.keyPressed(key, nativeKey)) {
				return true;
			}
		}
		return super.keyPressed(key, nativeKey);
	}

	@Override
	public void tick() {
		tab.tick(new Host() {
			@Override
			public Widget add(Widget w) {
				return TabPage.this.add(w);
			}

			@Override
			public void rebuild() {
				TabPage.this.rebuild();
			}
		});
		String state = tab.state();
		if (state != null && !state.equals(shownState) && (!typing() || tab.rebuildWhileTyping())) {
			rebuild();
		}
	}

	/** Right Shift: straight back to the game. */
	@Override
	protected boolean rightShift() {
		close();
		return true;
	}

	@Override
	public void close() {
		ArcticClient.saveConfig();
		super.close();
	}
}
