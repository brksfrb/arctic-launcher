package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.DimGfx;
import com.arcticlauncher.client.gfx.BrandMark;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.hud.Hud;
import com.arcticlauncher.client.hud.HudWidget;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.Page;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;

import com.arcticlauncher.client.Lang;
/**
 * Right Shift in a world: your HUD, live and movable. Drag a widget to move
 * it (it snaps to edges and to other widgets), drag its bottom-right corner
 * or scroll on it to resize it, right-click it for its settings. The card in the middle opens Mods and the
 * other Arctic screens. A widget under the card wins the mouse (the card dims then, and while one is being
 * moved); the card moves by its header, and double-clicking the header puts it back in the middle.
 */
public final class HudEditor extends Page {
	private static final float SCALE_STEP = 0.1f;
	private static final int DRAG_START = 3;
	private static final float FADE_SPEED = 6f;
	private static final int HIGHLIGHT = Hud.GAP / 2;
	private static final int BAR_W = 300;
	private static final int BAR_H = 74;
	private static final int MODS_H = 24;
	private static final int SMALL_H = 16;
	private static final int GAP = 4;
	private static final int PAD = 8;
	/** The card's header row (title and version): the handle that moves it. */
	private static final int HEADER_H = 20;
	private static final float DIMMED = 0.3f;
	private static final long DOUBLE_CLICK_MS = 400;
	private static final int MENU_W = 96;
	/** The resize corner: this many pixels in from a widget's bottom-right. */
	private static final int HANDLE = 6;
	/** Space either side of the help line inside its dark box. */
	private static final int HINT_PAD = 8;
	private static final float SCALE_SNAP = 0.05f;
	private static final String[][] SHORTCUTS = {
			{"mods.waypoints", "waypoints"}, {"mods.replays", "replays"}, {"mods.packs", "packs"}, {"mods.friends", "friends"},
	};

	private int bx;
	private int by;
	/** Card opacity: 1 normally, faded while a widget is moved or under the mouse. */
	private float shown = 1f;
	private long lastFrame;

	/** Being dragged. */
	private HudWidget dragging;
	private boolean moved;
	private int pressX;
	private int pressY;
	private int grabX;
	private int grabY;
	private int mouseX;
	private int mouseY;
	/** Being resized from its corner: its top-left and unscaled size. */
	private HudWidget resizing;
	private int resizeX;
	private int resizeY;
	private float baseW;
	private float baseH;
	/** Right-click menu for this widget, at (menuX, menuY). */
	private HudWidget menuFor;
	private int menuX;
	private int menuY;
	/** The right-click menu is the card's (reset), not a widget's. */
	private boolean cardMenu;
	/** The card is being moved by its header; the grab point inside it. */
	private boolean cardDragging;
	private int cardGrabX;
	private int cardGrabY;
	private long lastHeaderClick;
	/** The real mouse (the card's widgets are shown a faraway one while a HUD widget has it). */
	private int realX;
	private int realY;

	private final List<Button> barButtons = new ArrayList<Button>();
	private Map<HudWidget, int[]> rects = new java.util.HashMap<HudWidget, int[]>();
	private final Snapper snapper = new Snapper();

	@Override
	public boolean dimWorld() {
		return false;
	}

	/** The widgets are where they'll be on the HUD: no easing in. */
	@Override
	protected boolean animatesIn() {
		return false;
	}

	@Override
	public boolean pausesGame() {
		return false;
	}

	@Override
	protected void build() {
		com.arcticlauncher.client.config.ClientConfig cfg = ArcticClient.config();
		bx = clamp((width - BAR_W) / 2 + cfg.hudCardDx, width - BAR_W);
		by = clamp((height - BAR_H) / 2 + cfg.hudCardDy, height - BAR_H);
		barButtons.clear();
		Button mods = new Button(Lang.t("mods.mods"), Menus::mods).primary();
		add(mods).bounds(bx + PAD, by + 22, BAR_W - PAD * 2, MODS_H);
		barButtons.add(mods);
		java.util.List<String[]> shortcuts = new java.util.ArrayList<String[]>();
		for (String[] shortcut : SHORTCUTS) {
			// Not every version can play replays.
			if (!"replays".equals(shortcut[1]) || com.arcticlauncher.client.replay.Replays.canWatch()) {
				shortcuts.add(shortcut);
			}
		}
		int n = shortcuts.size();
		int sw = (BAR_W - PAD * 2 - GAP * (n - 1)) / n;
		for (int i = 0; i < n; i++) {
			final String id = shortcuts.get(i)[1];
			Button b = new Button(Lang.t(shortcuts.get(i)[0]), () -> Menus.open(id, this));
			add(b).bounds(bx + PAD + i * (sw + GAP), by + 22 + MODS_H + GAP, sw, SMALL_H);
			barButtons.add(b);
		}
		if (menuFor != null || cardMenu) {
			buildMenu();
		}
	}

	/** Keep the card fully on screen. */
	private static int clamp(int v, int max) {
		return Math.max(0, Math.min(Math.max(0, max), v));
	}

	private void setCard(int x, int y) {
		bx = clamp(x, width - BAR_W);
		by = clamp(y, height - BAR_H);
		com.arcticlauncher.client.config.ClientConfig cfg = ArcticClient.config();
		cfg.hudCardDx = bx - (width - BAR_W) / 2;
		cfg.hudCardDy = by - (height - BAR_H) / 2;
		rebuild();
	}

	/** Right-click on a widget: its settings, or take it off. */
	private void buildMenu() {
		final HudWidget w = menuFor;
		int x = Math.min(menuX, width - MENU_W - 2);
		int y = Math.min(menuY, height - 42);
		if (w == null) {
			add(new Button(Lang.t("mods.reset_card"), () -> {
				cardMenu = false;
				setCard((width - BAR_W) / 2, (height - BAR_H) / 2);
				ArcticClient.saveConfig();
			})).bounds(x, y, MENU_W, 18);
			return;
		}
		add(new Button(Lang.t("mods.settings"), () -> {
			menuFor = null;
			Menus.open("hud." + w.id, this);
		})).bounds(x, y, MENU_W, 18);
		add(new Button(Lang.t("mods.remove"), () -> {
			hide(w);
			menuFor = null;
			rebuild();
		})).bounds(x, y + 20, MENU_W, 18);
	}

	private boolean inBar(double mx, double my) {
		return mx >= bx && my >= by && mx < bx + BAR_W && my < by + BAR_H;
	}

	private boolean inHeader(double mx, double my) {
		return inBar(mx, my) && my < by + HEADER_H;
	}

	private boolean busy() {
		return resizing != null || cardDragging || (dragging != null && moved);
	}

	private boolean inMenu(double mx, double my) {
		return (menuFor != null || cardMenu) && mx >= Math.min(menuX, width - MENU_W - 2) && mx < Math.min(menuX, width - MENU_W - 2) + MENU_W
				&& my >= Math.min(menuY, height - 42) && my < Math.min(menuY, height - 42) + 38;
	}

	@Override
	public void render(Gfx g, int mx, int my) {
		long now = System.nanoTime();
		float dt = lastFrame == 0 ? 0f : Math.min(0.1f, (now - lastFrame) / 1e9f);
		lastFrame = now;
		realX = mx;
		realY = my;
		// A HUD widget has the mouse wherever it is, even under the card: the card is dimmed then.
		HudWidget under = busy() || inMenu(mx, my) ? null : placedAt(mx, my);
		boolean dim = busy() || (under != null && inBar(mx, my));
		float target = dim ? DIMMED : 1f;
		shown += (target - shown) * Math.min(1f, dt * FADE_SPEED);
		boolean away = busy() || under != null;
		super.render(g, away ? -10000 : mx, away ? -10000 : my);
	}

	@Override
	protected Gfx widgetGfx(Gfx g) {
		return shown < 0.99f ? new DimGfx(g, shown) : g;
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		Hud hud = ArcticClient.hud();
		// Laid out as the editor shows it (the boss bar's stand-in has its own place).
		hud.previewing(true);
		snapper.clear();
		if (dragging != null && moved) {
			placeDragged(g, hud);
		}
		if (resizing != null) {
			resize(g, hud);
		}
		rects = hud.layout(g);
		for (Map.Entry<HudWidget, int[]> e : hud.sorted(rects)) {
			HudWidget w = e.getKey();
			int[] r = e.getValue();
			boolean over = w == dragging || w == resizing || w == menuFor || (dragging == null && resizing == null && !cardDragging && !inMenu(realX, realY) && inside(r, realX, realY));
			// One pixel out: half the gap between stacked widgets, so boxes touch but never overlap.
			int o = HIGHLIGHT;
			g.fill(r[0] - o, r[1] - o, r[0] + r[2] + o, r[1] + r[3] + o, Draw.alpha(0xFFFFFFFF, over ? 0.12f : 0.04f));
			Draw.outline(g, r[0] - o, r[1] - o, r[0] + r[2] + o, r[1] + r[3] + o, 1, Draw.alpha(0xFFFFFFFF, over ? 0.9f : 0.3f));
			hud.draw(g, s, w, r, true);
			if (over) {
				// The resize corner: a small triangle at the bottom-right.
				int cx = r[0] + r[2] + o;
				int cy = r[1] + r[3] + o;
				for (int k = 0; k < HANDLE; k++) {
					g.fill(cx - k - 1, cy - HANDLE + k, cx, cy - HANDLE + k + 1, s.accent);
				}
			}
		}
		snapper.draw(g, s, width, height);
		drawBar(shown < 0.99f ? new DimGfx(g, shown) : g, s);
		String help = Lang.t("mods.drag_to_move_drag_the_corner_or_scroll_to");
		int hw = g.textWidth(help) / 2 + HINT_PAD;
		// Under the menu bar: at the bottom it would sit on the hotbar.
		int hy = by + BAR_H + 10;
		if (hy + 13 > height) {
			hy = by - 18;
		}
		int hx = Math.max(hw, Math.min(width - hw, bx + BAR_W / 2));
		Draw.round(g, hx - hw, hy - 5, hx + hw, hy + 13, 4, Draw.alpha(0xC0000000, shown));
		Draw.centered(g, help, hx, hy, Draw.alpha(0xFFFFFFFF, shown), false);
	}

	private void drawBar(Gfx g, Style s) {
		Skin.panel(g, s, bx, by, bx + BAR_W, by + BAR_H);
		BrandMark.draw(g, bx + PAD, by + 6, 12);
		g.text("§lArctic", bx + PAD + 16, by + 8, s.text, false); // i18n-ignore: the brand
		String version = ArcticClient.platform().minecraftVersion();
		g.text(version, bx + BAR_W - PAD - g.textWidth(version), by + 8, s.muted, false);
	}

	/** Scale the widget so its bottom-right corner follows the mouse (top-left stays). */
	private void resize(Gfx g, Hud hud) {
		float k = Math.max((mouseX - resizeX) / baseW, (mouseY - resizeY) / baseH);
		k = Math.round(k / SCALE_SNAP) * SCALE_SNAP;
		k = Math.max(Hud.MIN_SCALE, Math.min(Hud.MAX_SCALE, k));
		hud.slot(resizing).scale = k;
		int[] rect = {resizeX, resizeY, Math.round(baseW * k), Math.round(baseH * k)};
		hud.place(g, resizing, rect, resizeX, resizeY);
	}

	private static boolean onHandle(int[] r, double mx, double my) {
		return mx >= r[0] + r[2] - HANDLE && mx <= r[0] + r[2] + 2 && my >= r[1] + r[3] - HANDLE && my <= r[1] + r[3] + 2;
	}

	/** Follow the mouse, snapping to the screen and the other widgets. */
	private void placeDragged(Gfx g, Hud hud) {
		int[] me = rects.get(dragging);
		if (me == null) {
			return;
		}
		List<int[]> others = new ArrayList<int[]>();
		for (Map.Entry<HudWidget, int[]> e : rects.entrySet()) {
			if (e.getKey() != dragging) {
				others.add(e.getValue());
			}
		}
		int x = snapper.x(mouseX - grabX, me[2], width, others);
		int y = snapper.y(mouseY - grabY, me[3], height, others);
		hud.place(g, dragging, me, x, y);
	}

	private static boolean inside(int[] r, double mx, double my) {
		return mx >= r[0] && my >= r[1] && mx < r[0] + r[2] && my < r[1] + r[3];
	}

	private HudWidget placedAt(double mx, double my) {
		HudWidget found = null;
		for (HudWidget w : ArcticClient.hud().widgets()) {
			int[] r = rects.get(w);
			if (r != null && inside(r, mx, my)) {
				found = w;
			}
		}
		return found;
	}

	@Override
	public boolean mouseClicked(double mx, double my, int button) {
		if (busy()) {
			return true;
		}
		boolean menuHit = inMenu(mx, my);
		HudWidget w = menuHit ? null : placedAt(mx, my);
		// A widget under the card wins; the card's buttons only take clicks where none is.
		if (w == null && super.mouseClicked(mx, my, button)) {
			return true;
		}
		if (menuFor != null || cardMenu) {
			menuFor = null;
			cardMenu = false;
			rebuild();
		}
		if (w == null) {
			if (!inBar(mx, my)) {
				return false;
			}
			if (inHeader(mx, my)) {
				if (button == Keys.MOUSE_RIGHT) {
					cardMenu = true;
					menuX = (int) mx;
					menuY = (int) my;
					rebuild();
					return true;
				}
				long now = System.currentTimeMillis();
				if (now - lastHeaderClick < DOUBLE_CLICK_MS) {
					lastHeaderClick = 0;
					setCard((width - BAR_W) / 2, (height - BAR_H) / 2);
					ArcticClient.saveConfig();
					return true;
				}
				lastHeaderClick = now;
				cardDragging = true;
				cardGrabX = (int) mx - bx;
				cardGrabY = (int) my - by;
			}
			return true;
		}
		if (button == Keys.MOUSE_RIGHT) {
			menuFor = w;
			menuX = (int) mx;
			menuY = (int) my;
			rebuild();
			return true;
		}
		int[] r = rects.get(w);
		if (onHandle(r, mx, my)) {
			float scale = ArcticClient.hud().slot(w).scale;
			resizing = w;
			resizeX = r[0];
			resizeY = r[1];
			baseW = Math.max(1f, r[2] / scale);
			baseH = Math.max(1f, r[3] / scale);
			mouseX = (int) mx;
			mouseY = (int) my;
			return true;
		}
		dragging = w;
		moved = false;
		pressX = (int) mx;
		pressY = (int) my;
		grabX = (int) mx - r[0];
		grabY = (int) my - r[1];
		mouseX = pressX;
		mouseY = pressY;
		return true;
	}

	@Override
	public boolean mouseDragged(double mx, double my, int button) {
		if (cardDragging) {
			setCard((int) mx - cardGrabX, (int) my - cardGrabY);
			return true;
		}
		if (resizing != null) {
			mouseX = (int) mx;
			mouseY = (int) my;
			return true;
		}
		if (dragging == null) {
			return super.mouseDragged(mx, my, button);
		}
		mouseX = (int) mx;
		mouseY = (int) my;
		if (!moved && (Math.abs(mx - pressX) > DRAG_START || Math.abs(my - pressY) > DRAG_START)) {
			moved = true;
		}
		return true;
	}

	@Override
	public boolean mouseReleased(double mx, double my, int button) {
		if (cardDragging) {
			cardDragging = false;
			ArcticClient.saveConfig();
			return true;
		}
		if (resizing != null) {
			resizing = null;
			ArcticClient.saveConfig();
			return true;
		}
		if (dragging == null) {
			return super.mouseReleased(mx, my, button);
		}
		dragging = null;
		moved = false;
		ArcticClient.saveConfig();
		return true;
	}

	private void hide(HudWidget w) {
		HudSlot slot = ArcticClient.hud().slot(w);
		slot.enabled = false;
		// Showing it again lays it out where it would have gone, not where it was left.
		slot.placed = false;
		ArcticClient.saveConfig();
	}

	@Override
	public boolean mouseScrolled(double mx, double my, double amount) {
		HudWidget w = placedAt(mx, my);
		if (w == null) {
			return false;
		}
		HudSlot slot = ArcticClient.hud().slot(w);
		float next = Math.round((slot.scale + (amount > 0 ? SCALE_STEP : -SCALE_STEP)) * 10) / 10f;
		slot.scale = Math.max(Hud.MIN_SCALE, Math.min(Hud.MAX_SCALE, next));
		ArcticClient.saveConfig();
		return true;
	}

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
