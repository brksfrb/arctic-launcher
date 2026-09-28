package com.arcticlauncher.client.feature;

import java.util.HashSet;
import java.util.List;
import java.util.Set;

import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.notice.Notices;

/** A pop-up when worn armor or the held tool is about to break (once per item until repaired). */
public final class DurabilityWatch {
	/** Warn at or below this share of durability left. */
	private static final double LOW = 0.10;
	/** Checked this often (ticks). */
	private static final int EVERY = 20;

	private final Set<String> warned = new HashSet<String>();
	private int ticks;

	public void tick(Platform platform) {
		if (++ticks % EVERY != 0) {
			return;
		}
		Set<String> low = new HashSet<String>();
		List<Object[]> items = platform.durability();
		for (Object[] item : items) {
			String name = (String) item[0];
			int left = (Integer) item[1];
			int max = (Integer) item[2];
			String key = item[3] + ":" + name;
			if (max > 0 && left <= max * LOW) {
				low.add(key);
				if (warned.add(key)) {
					Notices.post(name + " is about to break", left + " of " + max + " durability left");
				}
			}
		}
		// Repaired or swapped: warn again next time it gets low.
		warned.retainAll(low);
	}
}
