package com.arcticlauncher.client.menu;

import java.util.List;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.SoundRule;
import com.arcticlauncher.client.feature.SoundRules;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.TextField;

/** The Sound Changer page: a piece of a sound's id and the volume it plays at. */
final class SoundForms {
	private static final int FIELD_H = 20;
	private static final int GAP = 4;
	private static final int PERCENT_W = 44;
	private static final int REMOVE_W = 22;

	private SoundForms() {}

	static void build(final Host host, Form f) {
		final List<SoundRule> list = ArcticClient.config().soundRules;
		f.section("Sounds");
		f.note("Part of the sound's name, then its volume in percent (0 = silent). Names look like entity.player.hurt,");
		f.note("block.grass.step, weather.rain, entity.experience_orb.pickup. The first rule that fits is used.");
		f.gap(4);
		for (final SoundRule r : list) {
			int top = f.row(FIELD_H + GAP);
			final TextField match = new TextField("step, hurt, rain", 40, TextField.ANY).text(r.match);
			match.onChange(() -> r.match = match.text());
			f.put(match, f.x, top, f.w - PERCENT_W - REMOVE_W - GAP * 2, FIELD_H);
			final TextField percent = new TextField("%", 3, TextField.DIGITS).text(String.valueOf(r.percent));
			percent.onChange(() -> {
				try {
					r.percent = Math.min(100, Integer.parseInt(percent.text()));
				} catch (NumberFormatException e) {
					r.percent = 100;
				}
			});
			f.put(percent, f.x + f.w - PERCENT_W - REMOVE_W - GAP, top, PERCENT_W, FIELD_H);
			f.put(new Button("×", () -> {
				list.remove(r);
				ArcticClient.saveConfig();
				host.rebuild();
			}), f.x + f.w - REMOVE_W, top, REMOVE_W, FIELD_H);
		}
		if (list.size() < SoundRules.MAX) {
			int top = f.row(FIELD_H + GAP);
			f.put(new Button("Add a sound", () -> {
				SoundRule r = new SoundRule();
				r.percent = 0;
				list.add(r);
				ArcticClient.saveConfig();
				host.rebuild();
			}).primary(), f.x, top, 110, FIELD_H);
		}
	}
}
