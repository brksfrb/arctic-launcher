package com.arcticlauncher.mod;

//#if MC < 1.16
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.TextComponent;

/** A wrapped chat row with a fixed render offset and a separate hit-test gap. */
public final class ChatStampLegacyLine extends TextComponent {
	public final Component stamp;
	public final Component body;
	public final int slot;

	public ChatStampLegacyLine(Component stamp, Component body, int slot, int stampHitWidth) {
		super("");
		this.stamp = stamp;
		this.body = body;
		this.slot = slot;
		if (stamp != null) append(stamp);
		append(new Gap(slot - stampHitWidth));
		append(body);
	}

	public static final class Gap extends TextComponent {
		public final int width;

		private Gap(int width) {
			super("");
			this.width = width;
		}

		@Override
		public Gap copy() {
			// Vanilla flattens copied components before testing their click widths.
			return new Gap(width);
		}
	}
}
//#endif
