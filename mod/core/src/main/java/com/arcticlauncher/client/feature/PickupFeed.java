package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.Iterator;
import java.util.List;
import java.util.Map;

/**
 * Lists what you pick up ("+64 Diamond") at the right edge for a moment. It watches what you carry:
 * a kind that grows between two ticks was picked up (this works the same on every version; what
 * you take out of a chest shows too, but not while a screen is open, nor in the first moments of a
 * world while the game is still filling your inventory).
 */
public final class PickupFeed {
	private static final int MAX_ROWS = 5;
	private static final long SHOW_MS = 3500;
	private static final long FADE_MS = 600;
	/** Ticks to just look after joining or closing a screen, before telling pickups apart. */
	private static final int SETTLE_TICKS = 20;
	private static final int ROW_H = 18;

	private final List<Row> rows = new ArrayList<Row>();
	private Map<String, Integer> before;
	private int settle = SETTLE_TICKS;

	private static final class Row {
		final String key;
		final Object stack;
		final String name;
		int count;
		long at;

		Row(String key, Object stack, String name, int count, long at) {
			this.key = key;
			this.stack = stack;
			this.name = name;
			this.count = count;
			this.at = at;
		}
	}

	/** Every client tick. */
	public void tick(Platform platform, boolean screenOpen) {
		if (!ArcticClient.config().pickupFeed || !platform.inWorld() || screenOpen) {
			settle = SETTLE_TICKS;
			before = null;
			if (!platform.inWorld()) {
				rows.clear();
			}
			return;
		}
		if (settle > 0) {
			settle--;
		}
		List<Object[]> carried = platform.carried();
		Map<String, Integer> now = new HashMap<String, Integer>();
		for (Object[] item : carried) {
			now.put((String) item[0], (Integer) item[3]);
		}
		if (before != null && settle == 0) {
			long time = System.currentTimeMillis();
			for (Object[] item : carried) {
				Integer had = before.get((String) item[0]);
				int gained = (Integer) item[3] - (had == null ? 0 : had);
				if (gained > 0) {
					add((String) item[0], item[1], (String) item[2], gained, time);
				}
			}
		}
		before = now;
	}

	private void add(String key, Object stack, String name, int count, long time) {
		for (Row row : rows) {
			if (row.key.equals(key)) {
				row.count += count;
				row.at = time;
				return;
			}
		}
		rows.add(0, new Row(key, stack, name, count, time));
		while (rows.size() > MAX_ROWS) {
			rows.remove(rows.size() - 1);
		}
	}

	/** Draw the rows, newest on top, above the hotbar at the right edge. */
	public void render(Gfx g, Style s) {
		if (rows.isEmpty() || !ArcticClient.config().pickupFeed) {
			return;
		}
		long now = System.currentTimeMillis();
		int y = g.height() - 70;
		for (Iterator<Row> it = rows.iterator(); it.hasNext();) {
			Row row = it.next();
			long age = now - row.at;
			if (age > SHOW_MS) {
				it.remove();
				continue;
			}
			float fade = age < SHOW_MS - FADE_MS ? 1f : (SHOW_MS - age) / (float) FADE_MS;
			int alpha = Math.max(0, Math.min(255, Math.round(fade * 255)));
			String text = "+" + row.count + " " + row.name;
			int w = g.textWidth(text) + 28;
			int x1 = g.width() - 4;
			int x0 = x1 - w;
			Draw.round(g, x0, y - ROW_H, x1, y, 4, (alpha * 3 / 4) << 24 | (s.panel & 0xFFFFFF));
			g.item(row.stack, x0 + 3, y - ROW_H + 1);
			g.text(text, x0 + 24, y - ROW_H + 5, alpha << 24 | (s.text & 0xFFFFFF), false);
			y -= ROW_H + 2;
		}
	}
}
