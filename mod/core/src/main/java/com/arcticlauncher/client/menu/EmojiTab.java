package com.arcticlauncher.client.menu;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.feature.EmoteAtlas;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Button;

import com.arcticlauncher.client.Lang;
/** The same approved pictures as chat; copying preserves the readable shortcode. */
final class EmojiTab implements MenuTab {
	public String title() { return Lang.t("mods.chat_emotes"); }
	public String hint() { return Lang.t("mods.click_a_picture_to_copy_its_shortcode_then"); }
	public void build(Host host, int x, int top, int w, int bottom) {
		final ClientConfig c = ArcticClient.config();
		Form f = new Form(host, x, top, w, bottom);
		f.section(Lang.t("mods.typing"));
		f.toggle(Lang.t("mods.chat_button"), Lang.t("mods.the_button_opens_the_picture_picker"), () -> c.emojiButton, on -> c.emojiButton = on);
		f.toggle(Lang.t("mods.chat_emotes"), Lang.t("mods.show_pictures_for_shortcodes_send_readable"), () -> c.chatEmoji, on -> c.chatEmoji = on);
		f.note(Lang.t("mods.twemoji_by_twitter_cc_by_4_0_arctic_mark_and"));
		int perRow = Math.max(1, f.w / 25), column = 0, y = 0;
		String group = "";
		for (final EmoteAtlas.Emote emote : EmoteAtlas.all()) {
			if (!group.equals(emote.group)) { f.section(Lang.t(emote.group)); column = 0; group = emote.group; }
			if (column == 0) y = f.row(25);
			f.put(new Button("", () -> {
				ArcticClient.platform().setClipboard(emote.shortcode());
				com.arcticlauncher.client.notice.Notices.post(Lang.t("mods.copied") + emote.shortcode(), Lang.t("mods.paste_into_chat_with_ctrl_v"));
			}) {
				@Override protected void draw(Gfx g, Style s, int mx, int my, float dt) {
					super.draw(g, s, mx, my, dt);
					emote.draw(g, x + 2, y + 2, 18);
				}
			}, f.x + column * 25, y, 22, 22);
			column = (column + 1) % perRow;
		}
		f.done();
	}
}
