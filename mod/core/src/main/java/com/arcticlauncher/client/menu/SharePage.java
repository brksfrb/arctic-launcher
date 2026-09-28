package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.share.ShareService;
import com.arcticlauncher.client.style.Skin;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.Page;
import com.arcticlauncher.client.ui.TextField;

/** Use a HUD layout or crosshair someone shared: paste the code or text. */
public final class SharePage extends Page {
	private static final int W = 300;
	private static final int H = 128;
	/** Long enough for share text of a full HUD. */
	private static final int MAX_INPUT = 16384;
	private static final int WARN = 0xFFF87171;

	private TextField input;
	private int px;
	private int py;

	@Override
	protected void build() {
		px = (width - W) / 2;
		py = (height - H) / 2;
		String typed = input == null ? "" : input.text();
		input = add(new TextField("Code like abcd-efgh, or share text (Ctrl+V)", MAX_INPUT, TextField.ANY).text(typed));
		input.bounds(px + 12, py + 44, W - 24, 18);
		focus(input);
		add(new Button("Use it", new Runnable() {
			@Override
			public void run() {
				ShareService.use(input.text(), new Runnable() {
					@Override
					public void run() {
						// Back to the tab it was opened from; the menu shows what changed.
						close();
					}
				});
			}
		}).primary()).bounds(px + W - 12 - 80, py + H - 30, 80, 20);
		add(new Button("Back", new Runnable() {
			@Override
			public void run() {
				ShareService.clearStatus();
				close();
			}
		})).bounds(px + 12, py + H - 30, 70, 20);
	}

	@Override
	protected void drawBehind(Gfx g, Style s, int mx, int my) {
		Skin.panel(g, s, px, py, px + W, py + H);
		g.text("Use a code", px + 12, py + 12, s.text, false);
		g.text(Draw.fit(g, "A HUD layout, crosshair or client settings from a friend.", W - 24, px + 12, py + 26), px + 12, py + 26, s.muted, false);
		String status = ShareService.status();
		if (!status.isEmpty()) {
			g.text(Draw.fit(g, status, W - 24, px + 12, py + 70), px + 12, py + 70, ShareService.problem() ? WARN : s.accent, false);
		}
	}

	/** Leaving without using a code clears its message; after using one, the menu shows it. */
	@Override
	public void close() {
		super.close();
	}
}
