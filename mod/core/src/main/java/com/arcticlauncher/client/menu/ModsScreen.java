package com.arcticlauncher.client.menu;

import java.util.ArrayList;
import java.util.List;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Keys;
import com.arcticlauncher.client.Lang;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.KeyButton;
import com.arcticlauncher.client.ui.Page;
import com.arcticlauncher.client.ui.Scroll;
import com.arcticlauncher.client.ui.TextField;
import com.arcticlauncher.client.ui.Toggle;
import com.arcticlauncher.client.ui.Widget;

/** Catalog, recent features and the settings for the selected card. */
final class ModsScreen extends Page {
	@Override protected boolean arcticMenuChrome() { return true; }
	private static final int GAP = 6;
	private static final int DETAIL_W = 168;
	private static final int RECENT_MAX = 4;
	private static final String[] FIRST = {
			"armorhider", "zoom", "freelook", "sprint", "sneak", "waypoints", "screenshots",
			"replays", "death", "durability", "leave", "reconnect"
	};
	private static final List<String> RECENT = new ArrayList<>();
	private static Mod.Category category = Mod.Category.ALL;
	private static String hudGroup = "mods.hud_group.all";

	private final TextField search = new TextField(Lang.t("mods.search_mods"), 32, TextField.ANY);
	private final List<KeyButton> keys = new ArrayList<>();
	private MenuFrame frame;
	private String selectedId = FIRST[0];
	private Scroll grid;
	private Scroll details;
	private int gridTop, detailX, detailTop;
	private Mod selected;
	private boolean wide;
	private String emptyMessage;
	private boolean resetGrid;
	private String previousDetail;
	private int detailOffset;

	ModsScreen() {
		search.onChange(() -> {
			resetGrid = true;
			rebuild();
		});
	}

	@Override public boolean pausesGame() { return false; }
	@Override protected boolean typesAnywhere() { return true; }

	@Override
	protected void build() {
		int offset = resetGrid || grid == null ? 0 : grid.offset();
		resetGrid = false;
		keys.clear();
		detailOffset = details == null ? 0 : details.offset();
		details = null;
		frame = new MenuFrame(width, height);
		int x = frame.content, end = frame.end();
		int searchW = Math.min(120, Math.max(34, (end - x) / 3));
		int searchX = end - 20 - searchW;
		add(search).bounds(searchX, frame.top, searchW, 16);
		add(new Button("×", this::close)).bounds(end - 16, frame.top, 16, 16);
		if (ArcticClient.platform().hasFeatures() && searchX - x >= 90) {
			add(new Button(Lang.t("mods.edit_hud"), () -> ArcticClient.platform().openPage(new HudEditor())))
					.bounds(searchX - 62, frame.top, 56, 16);
		}
		Host navigation = host(0);
		int rows = frame.navigation(navigation, category, this::selectCategory, () -> Menus.open("account", this));
		if (end - x >= 220 && ArcticClient.platform().hasFeatures() && category == Mod.Category.CLIENT) {
			add(new Button(Lang.t("mods.presets"), () -> ArcticClient.platform().openPage(new Presets())))
					.bounds(x, rows, 56, 16);
			rows += 20;
		}
		if (category == Mod.Category.HUD) {
			int cx = x;
			for (String group : ModCatalog.HUD_GROUPS) {
				String label = Lang.t(group);
				int cw = Math.min(end - x, Math.max(30, label.length() * 5 + 8));
				if (cx + cw > end) {
					cx = x;
					rows += 19;
				}
				add(new Button(label, () -> {
					hudGroup = group;
					resetGrid = true;
					rebuild();
				}).selected(group.equals(hudGroup))).bounds(cx, rows, cw, 16);
				cx += cw + 4;
			}
			rows += 20;
		}
		List<Mod> mods = visible();
		selected = find(mods, selectedId);
		if (selected == null && !mods.isEmpty()) selected = mods.get(0);
		wide = end - x >= 440 && frame.bottom - rows >= 100;
		detailX = end - DETAIL_W;
		int gridEnd = wide ? detailX - 8 : end;
		gridTop = recent(x, rows, gridEnd);
		detailTop = gridTop;
		int bottom = Math.max(gridTop + 1, frame.bottom - 8);
		int gridW = Math.max(1, gridEnd - x);
		grid = new Scroll(x, gridTop, gridEnd, bottom, offset);
		add(grid.bar());
		int cols = Math.max(1, Math.min(wide ? 3 : 5, (gridW + GAP) / 84));
		int cardW = (gridW - (cols - 1) * GAP) / cols;
		int cardH = wide ? 63 : 71;
		for (int i = 0; i < mods.size(); i++) {
			Mod mod = mods.get(i);
			ModCard card = new ModCard(mod, wide && mod == selected, () -> choose(mod), () -> open(mod));
			card.bounds(x + i % cols * (cardW + GAP), gridTop + i / cols * (cardH + GAP), cardW, cardH);
			grid.add(add(card));
		}
		grid.settle();
		emptyMessage = mods.isEmpty() ? Lang.t("mods.no_mods_match") + search.text() + "\"." : null;
		if (wide && selected != null) {
			if (!selected.id.equals(previousDetail)) detailOffset = 0;
			previousDetail = selected.id;
			buildDetails(selected, bottom);
		}
	}

	private Host host(int offset) {
		return new Host() {
			@Override public Widget add(Widget w) { return ModsScreen.this.add(w); }
			@Override public void rebuild() { ModsScreen.this.rebuild(); }
			@Override public void listenKeys(KeyButton button) { keys.add(button); }
			@Override public Scroll scroll(int x0, int y0, int x1, int y1) {
				details = new Scroll(x0, y0, x1, y1, offset);
				ModsScreen.this.add(details.bar());
				return details;
			}
		};
	}

	static Mod.Category category() { return category; }

	void selectCategory(Mod.Category next) {
		category = next;
		resetGrid = true;
		if (frame != null) rebuild();
	}

	private int recent(int x, int y, int end) {
		if (end - x < 160 || frame.bottom - y < 70) return y;
		add(new com.arcticlauncher.client.ui.Label(Lang.t("mods.recent"), com.arcticlauncher.client.ui.Label.Kind.MUTED))
				.bounds(x, y, 35, 16);
		int cx = x + 36;
		List<Mod> all = ModCatalog.all();
		List<String> ids = new ArrayList<>(RECENT);
		for (String id : new String[] {"armorhider", "zoom", "freelook", "waypoints"}) {
			if (!ids.contains(id)) ids.add(id);
		}
		int shown = 0;
		for (String id : ids) {
			if (shown == RECENT_MAX) break;
			Mod mod = find(all, id);
			if (mod == null) continue;
			int cw = Math.min(90, mod.name.length() * 5 + 22);
			if (cx + cw > end) break;
			add(new Widget() {
				@Override
				protected void draw(Gfx g, Style s, int mx, int my, float dt) {
					boolean active = selected != null && selected.id.equals(mod.id);
					int fill = active ? Draw.mix(s.button, s.buttonHover, 0.82f) : Draw.mix(s.field, s.buttonHover, hover);
					Skin.outlined(g, x, y, x + w, y + h, 2, fill, active ? s.accent : s.border);
					g.texture(mod.iconKey(), x + 4, y + 3, 10, 10, 0, 0, 32, 32, 32, 32);
					g.text(Draw.fit(g, mod.name, w - 19), x + 16, y + 4, s.text, false);
				}

				@Override
				public boolean click(double mx, double my, int button) {
					if (button != Keys.MOUSE_LEFT) return false;
					search.text("");
					category = Mod.Category.ALL;
					resetGrid = true;
					remember(mod);
					selectedId = mod.id;
					if (!wide && mod.hasSettings()) open(mod);
					else rebuild();
					return true;
				}
			}).bounds(cx, y, cw, 16);
			cx += cw + 4;
			shown++;
		}
		return y + 20;
	}

	private void buildDetails(Mod mod, int bottom) {
		int x = detailX + 5;
		int w = DETAIL_W - 10;
		if (mod.hasSwitch()) {
			add(new Toggle("", null, Form.binding(mod::on, mod::setOn)))
					.bounds(x + w - 31, detailTop + 3, 31, 18);
		}
		int top = detailTop + 39;
		if (mod.settings != null && mod.inlineSettings) {
			Host host = host(detailOffset);
			Form form = new Form(host, x - 2, top, w + 4, bottom - 26, true);
			mod.settings.build(host, form);
			form.done();
		}
		if (mod.hasSettings()) {
			int buttonY = mod.inlineSettings ? bottom - 20 : top + 3;
			add(new Button(Lang.t("mods.settings"), () -> open(mod))).bounds(x, buttonY, w, 16);
		}
	}

	private void choose(Mod mod) {
		remember(mod);
		if (wide) {
			if (!mod.id.equals(selectedId)) {
				selectedId = mod.id;
				rebuild();
			} else if (mod.hasSettings()) {
				open(mod);
			}
		} else if (mod.hasSettings()) {
			open(mod);
		}
	}

	private static void remember(Mod mod) {
		RECENT.remove(mod.id);
		RECENT.add(0, mod.id);
		if (RECENT.size() > RECENT_MAX) RECENT.remove(RECENT_MAX);
		ModCatalog.markSeen(mod.id);
	}

	private void open(Mod mod) {
		remember(mod);
		selectedId = mod.id;
		ArcticClient.platform().openPage(Menus.pageFor(mod, this));
	}

	private static Mod find(List<Mod> mods, String id) {
		for (Mod mod : mods) if (id.equals(mod.id)) return mod;
		return null;
	}

	private List<Mod> visible() {
		List<Mod> filtered = new ArrayList<>();
		for (Mod mod : ModCatalog.all()) {
			if (category != Mod.Category.ALL && !inCategory(mod.category)) continue;
			if (category == Mod.Category.HUD && !"mods.hud_group.all".equals(hudGroup) && !hudGroup.equals(mod.group)) continue;
			if (mod.matches(search.text())) filtered.add(mod);
		}
		if (category != Mod.Category.ALL || !search.text().trim().isEmpty()) return filtered;
		List<Mod> ordered = new ArrayList<>(filtered.size());
		for (String id : FIRST) {
			Mod mod = find(filtered, id);
			if (mod != null) {
				ordered.add(mod);
				filtered.remove(mod);
			}
		}
		ordered.addAll(filtered);
		return ordered;
	}

	private static boolean inCategory(Mod.Category value) {
		if (category == Mod.Category.GAMEPLAY) {
			return value == Mod.Category.GAMEPLAY || value == Mod.Category.COMBAT || value == Mod.Category.INVENTORY
					|| value == Mod.Category.MOVEMENT || value == Mod.Category.TOOLS;
		}
		return category == value || category == Mod.Category.SOCIAL && value == Mod.Category.PLAYERS;
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		frame.draw(g, s);
		g.text(Lang.t("mods.mods"), frame.content, frame.top + 3, s.text, false);
		g.fill(frame.content, frame.top + 20, frame.end(), frame.top + 21, s.border);
		if (emptyMessage != null) {
			Draw.centered(g, Draw.fit(g, emptyMessage, frame.end() - frame.content), (frame.content + frame.end()) / 2, gridTop + 20, s.muted, false);
		}
		if (wide && selected != null) {
			int right = detailX + DETAIL_W;
			Skin.inset(g, s, detailX, detailTop, right, frame.bottom - 8);
			g.texture(selected.iconKey(), detailX + 6, detailTop + 5, 13, 13, 0, 0, 32, 32, 32, 32);
			g.text(Draw.fit(g, selected.name, DETAIL_W - 55), detailX + 24, detailTop + 5, s.text, false);
			String description = selected.detailHint == null ? selected.description : selected.detailHint;
			g.text(Draw.fit(g, description, DETAIL_W - 12, detailX + 6, detailTop + 23),
					detailX + 6, detailTop + 23, s.muted, false);
			g.fill(detailX + 6, detailTop + 33, right - 6, detailTop + 34, s.border);
		}
		grid.drawBar(g, s);
		if (details != null) details.drawBar(g, s);
	}

	@Override
	public boolean mouseScrolled(double mx, double my, double amount) {
		return details != null && details.wheel(mx, my, amount) || grid != null && grid.wheel(mx, my, amount);
	}

	@Override
	public boolean keyPressed(int key, int nativeKey) {
		for (KeyButton button : keys) if (button.keyPressed(key, nativeKey)) return true;
		return super.keyPressed(key, nativeKey);
	}

	@Override
	public boolean charTyped(int codepoint) {
		if (!typing() && codepoint > ' ') focus(search);
		return super.charTyped(codepoint);
	}

	@Override protected boolean rightShift() { close(); return true; }
	@Override public void close() { ArcticClient.saveConfig(); super.close(); }
}
