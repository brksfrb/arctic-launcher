package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.notice.Notices;

/** Source-message metadata carried by the styles of every wrapped chat fragment. */
public final class ChatCopy {
	public interface Tagged {
		Data arctic$copyData();
		void arctic$copyData(Data data);
	}

	public static final class Data {
		public final String text;
		public final boolean target;

		public Data(String text, boolean target) {
			this.text = text.replaceAll("§[0-9A-FK-ORa-fk-or]", "");
			this.target = target;
		}

		@Override
		public boolean equals(Object other) {
			return other instanceof Data && target == ((Data) other).target && text.equals(((Data) other).text);
		}

		@Override
		public int hashCode() { return 31 * text.hashCode() + (target ? 1 : 0); }
	}

	private static long copiedNoticeAt;

	private ChatCopy() {}

	public static boolean enabled() {
		return ArcticClient.config() != null && ArcticClient.config().chatCopy && !ArcticClient.config().chatOff;
	}

	public static Data data(Object style) {
		return style instanceof Tagged ? ((Tagged) style).arctic$copyData() : null;
	}

	public static boolean click(Object style, int button, boolean shift) {
		Data data = data(style);
		if (!enabled() || data == null || !(button == 0 && data.target || button == 1 && shift)) {
			return false;
		}
		ArcticClient.platform().setClipboard(data.text);
		long now = System.currentTimeMillis();
		if (now - copiedNoticeAt >= 1000) {
			copiedNoticeAt = now;
			Notices.post("Copied", "Chat message copied to clipboard");
		}
		return true;
	}
}
