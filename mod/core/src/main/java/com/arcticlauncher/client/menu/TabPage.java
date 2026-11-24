package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.KeyButton;
import com.arcticlauncher.client.ui.Label;
import com.arcticlauncher.client.ui.Page;
import com.arcticlauncher.client.ui.Scroll;
import com.arcticlauncher.client.ui.Toggle;
import com.arcticlauncher.client.ui.Widget;

import com.arcticlauncher.client.Lang;
/**
 * One screen of the Arctic menu in the shared panel: a header with a back
 * arrow, the icon and title (and the mod's switch), and the tab's content.
 */
final class TabPage extends Page {
	@Override protected boolean arcticMenuChrome() { return true; }
	private static final int ICON = 16;
	private static final int SWITCH_W = 44;

	private final MenuTab tab;
	/** The mod this page belongs to (its switch shows in the header), or null. */
	private final Mod mod;
	/** Where the back arrow goes (the screen this was opened from); null = Mods. */
	private final Page back;
	private final java.util.List<KeyButton> keyButtons = new java.util.ArrayList<KeyButton>();
	private Scroll scroll;
	private MenuFrame frame;
	private int bodyTop;
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
		frame = new MenuFrame(width, height);
		px = frame.x;
		py = frame.y;
		pw = frame.right - px;
		ph = frame.bottom - py;
		keyButtons.clear();
		if (scroll != null) scrollOffset = scroll.offset();
		scroll = null;
		int rows = frame.navigation(new Host() {
			@Override public Widget add(Widget widget) { return TabPage.this.add(widget); }
			@Override public void rebuild() { TabPage.this.rebuild(); }
		}, ModsScreen.category(), next -> {
			ModsScreen catalog = back instanceof ModsScreen ? (ModsScreen) back : new ModsScreen();
			catalog.selectCategory(next);
			ArcticClient.platform().openPage(catalog);
		}, () -> {
			if (mod == null || !"account".equals(mod.id)) Menus.open("account", this);
		});
		bodyTop = rows + 3;
		add(new Button("<", () -> ArcticClient.platform().openPage(back != null ? back : new ModsScreen())))
				.bounds(frame.content, frame.top, 16, 16);
		if (mod != null && mod.hasSwitch()) {
			add(new Toggle("", null, Form.binding(mod::on, mod::setOn)))
					.bounds(frame.end() - SWITCH_W, frame.top, SWITCH_W, 16);
		}
		shownState = tab.state();
		int top = contentTop();
		if (hasToggleKey()) {
			buildToggleKey(top);
			top += Form.ROW + 8;
		}
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
				TabPage.this.add(scroll.bar());
				return scroll;
			}
		}, contentX(), top, contentW(), frame.bottom - 8);
	}

	/** Every feature with a switch gets a key that flips it (the ones with a key of their own already show it). */
	private boolean hasToggleKey() {
		return mod != null && mod.hasSwitch() && !mod.ownToggleKey;
	}

	/** The "Toggle key" row, laid out like a Form's key row. */
	private void buildToggleKey(int top) {
		final Mod m = mod;
		int x = contentX();
		int w = contentW();
		int keyWidth = 36;
		int textW = w - keyWidth - 8;
		add(new Label(Lang.t(m.toggleKeyLabel), Label.Kind.TEXT)).bounds(x, top + 3, textW, 9);
		final Label hint = add(new Label(Lang.t("mods.press_it_in_game_to_switch", m.name), Label.Kind.MUTED));
		hint.bounds(x, top + 14, textW, 9);
		final Label warn = add(new Label("", Label.Kind.ACCENT));
		warn.bounds(x, top + 14, textW, 9);
		final Runnable note = () -> {
			String other = ToggleKeys.usedBy(m, ToggleKeys.get(m));
			warn.text(other == null ? "" : Lang.t("mods.also_used_by") + other + Lang.t("mods.it_still_works"));
			warn.visible = other != null;
			hint.visible = other == null;
		};
		note.run();
		KeyButton button = new KeyButton(new KeyButton.Binding() {
			@Override
			public String get() {
				return ToggleKeys.get(m);
			}

			@Override
			public void set(String k) {
				ToggleKeys.set(m, k);
				note.run();
			}
		});
		add(button).bounds(x + w - keyWidth, top + 3, keyWidth, 16);
		keyButtons.add(button);
	}

	int contentX() {
		return frame.content;
	}

	int contentW() {
		return frame.end() - frame.content;
	}

	int contentTop() {
		return bodyTop;
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		frame.draw(g, s);
		int tx = frame.content + 23;
		String icon = mod != null ? mod.iconKey() : null;
		if (icon != null) {
			g.texture(icon, tx, frame.top + 1, ICON, ICON, 0, 0, 32, 32, 32, 32);
			tx += ICON + 3;
		}
		int room = frame.end() - tx - (mod != null && mod.hasSwitch() ? SWITCH_W + 8 : 0);
		g.text(Draw.fit(g, tab.title(), room), tx, frame.top + 1, s.text, false);
		String status = tab.status();
		String hint = status != null ? status : tab.hint();
		g.text(Draw.fit(g, hint, room), tx, frame.top + 12, status != null ? s.accent : s.muted, false);
		g.fill(frame.content, frame.top + 22, frame.end(), frame.top + 23, s.border);
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
