package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.CrosshairConfig;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.hud.Crosshair;
import com.arcticlauncher.client.share.ShareService;
import com.arcticlauncher.client.share.Shares;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;

import com.arcticlauncher.client.Lang;
/** The crosshair: shape, size, color and target colors, with a live preview. */
final class CrosshairTab implements MenuTab {
	private static final int PREVIEW = 80;
	private static final int GAP = 10;
	private static final int BTN_H = 18;
	private static final String[] SHAPES = {"mods.crosshair.shape.cross", "mods.crosshair.shape.dot", "mods.crosshair.shape.circle", "mods.crosshair.shape.both", "mods.crosshair.shape.picture"};

	private int bx;
	private int by;

	@Override
	public String title() {
		return Lang.t("mods.crosshair");
	}

	@Override
	public String hint() {
		return Lang.t("mods.your_own_crosshair_shape_size_color");
	}

	@Override
	public String status() {
		String s = ShareService.status();
		return s.isEmpty() ? null : s;
	}

	@Override
	public void build(final Host host, int x, int top, int w, int bottom) {
		final CrosshairConfig c = ArcticClient.config().crosshair;
		Form f = new Form(host, x, top, w - PREVIEW - GAP, bottom);
		f.section(Lang.t("mods.shape"));
		f.choice(Lang.t("mods.shape"), Lang.all(SHAPES), () -> indexOf(c.style), i -> c.style = CrosshairConfig.STYLES[i]);
		if ("image".equals(c.style)) {
			int row = f.row(BTN_H + 4);
			f.put(new Button("Open the folder for " + CrosshairConfig.IMAGE_FILE, () -> {
				com.arcticlauncher.client.feature.FolderOpener.open(Crosshair.imageFile().getParentFile());
			}), f.x, row, f.w, BTN_H);
		}
		f.stepper(Lang.t("mods.size"), () -> c.size, v -> c.size = (int) v, 1, 12, 1, "%.0f");
		f.stepper(Lang.t("mods.gap"), () -> c.gap, v -> c.gap = (int) v, 0, 8, 1, "%.0f");
		f.stepper(Lang.t("mods.thickness"), () -> c.thickness, v -> c.thickness = (int) v, 1, 4, 1, "%.0f");
		f.section(Lang.t("mods.color"));
		f.swatches(Lang.t("mods.color"), CrosshairConfig.COLORS, () -> c.color, v -> c.color = v);
		f.toggle(Lang.t("mods.outline"), Lang.t("mods.a_dark_edge_so_it_shows_on_bright_blocks"), () -> c.outline, on -> c.outline = on);
		f.toggle(Lang.t("mods.color_by_target"), Lang.t("mods.red_on_players_orange_on_hostile_mobs_green"),
				() -> c.targetColors, on -> c.targetColors = on);
		f.done();
		bx = x + w - PREVIEW;
		by = top;
		Button share = new Button(Lang.t("mods.share"), () -> ShareService.share(Shares.CROSSHAIR));
		share.enabled = !ShareService.busy();
		host.add(share).bounds(bx, by + PREVIEW + 8, PREVIEW, BTN_H);
		host.add(new Button(Lang.t("mods.use_a_code"), () -> ArcticClient.platform().openPage(new SharePage())))
				.bounds(bx, by + PREVIEW + 8 + BTN_H + 4, PREVIEW, BTN_H);
	}

	private static int indexOf(String style) {
		for (int i = 0; i < CrosshairConfig.STYLES.length; i++) {
			if (CrosshairConfig.STYLES[i].equals(style)) {
				return i;
			}
		}
		return 0;
	}

	/** The crosshair on a sky-and-grass backdrop, like in game. */
	@Override
	public void draw(Gfx g, Style s, int mx, int my) {
		g.gradient(bx, by, bx + PREVIEW, by + PREVIEW, 0xFF7FB2FF, 0xFFB5D3FF);
		g.fill(bx, by + PREVIEW * 2 / 3, bx + PREVIEW, by + PREVIEW, 0xFF5E9E3B);
		Draw.outline(g, bx, by, bx + PREVIEW, by + PREVIEW, 1, s.border);
		Crosshair.render(g, ArcticClient.config().crosshair, bx + PREVIEW / 2, by + PREVIEW / 2);
	}

	@Override
	public String state() {
		return String.valueOf(ShareService.busy());
	}
}
