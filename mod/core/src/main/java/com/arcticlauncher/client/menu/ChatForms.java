package com.arcticlauncher.client.menu;

import java.util.List;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.ClientConfig;
import com.arcticlauncher.client.config.QuickMessage;
import com.arcticlauncher.client.feature.QuickMessages;
import com.arcticlauncher.client.ui.Button;
import com.arcticlauncher.client.ui.KeyButton;
import com.arcticlauncher.client.ui.Label;
import com.arcticlauncher.client.ui.TextField;

/** Settings forms for the chat mods: quick messages, Auto GG, streamer mode. */
final class ChatForms {
	private static final int FIELD_H = 18;
	private static final int GAP = 4;
	private static final int REMOVE_W = 20;
	private static final int ADD_W = 120;
	private static final int SIDE_FIELD_W = 140;
	private static final int MAX_TEXT = 256;
	private static final int MAX_GG = 64;
	private static final int MAX_NAME = 16;

	private ChatForms() {}

	/** Keys that send a line: key, text, remove; then Add. */
	static void quickMessages(final Host host, Form f) {
		final List<QuickMessage> list = ArcticClient.config().quickMessages;
		f.section("Messages");
		f.note("Press the key to send the line. Start with / for a command.");
		f.note("Fills in: " + QuickMessages.PLACEHOLDERS);
		f.gap(4);
		for (final QuickMessage m : list) {
			int top = f.row(FIELD_H + GAP);
			KeyButton key = f.put(new KeyButton(new KeyButton.Binding() {
				@Override
				public String get() {
					return m.key;
				}

				@Override
				public void set(String k) {
					m.key = k;
					ArcticClient.saveConfig();
				}
			}), f.x, top, Form.KEY_W, FIELD_H);
			host.listenKeys(key);
			final TextField text = new TextField("Message, or /command", MAX_TEXT, TextField.ANY).text(m.text);
			text.onChange(() -> m.text = text.text());
			f.put(text, f.x + Form.KEY_W + GAP, top, f.w - Form.KEY_W - REMOVE_W - GAP * 2, FIELD_H);
			f.put(new Button("×", () -> {
				list.remove(m);
				ArcticClient.saveConfig();
				host.rebuild();
			}), f.x + f.w - REMOVE_W, top, REMOVE_W, FIELD_H);
		}
		if (list.size() < QuickMessages.MAX) {
			int top = f.row(FIELD_H + GAP);
			f.put(new Button("Add a message", () -> {
				QuickMessage m = new QuickMessage();
				m.text = list.isEmpty() ? "I'm at {x} {y} {z}" : "";
				list.add(m);
				ArcticClient.saveConfig();
				host.rebuild();
			}).primary(), f.x, top, ADD_W, FIELD_H);
		}
	}

	static void autoGg(Host host, Form f) {
		final ClientConfig c = ArcticClient.config();
		f.section("Message");
		int top = f.row(Form.ROW);
		f.put(new Label("What to say", Label.Kind.TEXT), f.x + 4, top, f.w - SIDE_FIELD_W - 8, Form.ROW);
		final TextField gg = new TextField("gg", MAX_GG, TextField.ANY).text(c.autoGgMessage);
		gg.onChange(() -> c.autoGgMessage = gg.text().trim().isEmpty() ? "gg" : gg.text());
		f.put(gg, f.x + f.w - SIDE_FIELD_W, top + (Form.ROW - FIELD_H) / 2, SIDE_FIELD_W, FIELD_H);
		f.note("Sent once when a game ends (Hypixel, Minemen and others).");
	}

	static void streamer(Host host, Form f) {
		final ClientConfig c = ArcticClient.config();
		f.section("Key");
		f.key("Streamer mode key", "Switches it on and off", Form.key(() -> c.streamerKey, k -> c.streamerKey = k));
		f.section("Name");
		int top = f.row(Form.ROW);
		f.put(new Label("Shown instead of yours", Label.Kind.TEXT), f.x + 4, top, f.w - SIDE_FIELD_W - 8, Form.ROW);
		final TextField name = new TextField("Streamer", MAX_NAME, TextField.ANY).text(c.streamerName);
		name.onChange(() -> c.streamerName = name.text().trim().isEmpty() ? "Streamer" : name.text().trim());
		f.put(name, f.x + f.w - SIDE_FIELD_W, top + (Form.ROW - FIELD_H) / 2, SIDE_FIELD_W, FIELD_H);
		f.note("Also hides your skin and the server address on screen.");
	}
}
