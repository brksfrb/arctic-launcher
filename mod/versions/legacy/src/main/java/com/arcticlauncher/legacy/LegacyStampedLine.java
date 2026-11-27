package com.arcticlauncher.legacy;

import net.minecraft.text.LiteralText;
import net.minecraft.text.Text;

/** A wrapped chat row. The empty component accounts for the gap during vanilla hit testing. */
public final class LegacyStampedLine extends LiteralText {
	public final Text stamp;
	public final Text body;
	public final int slot;

	public LegacyStampedLine(Text stamp, Text body, int slot, int stampWidth) {
		super("");
		this.stamp = stamp;
		this.body = body;
		this.slot = slot;
		if (stamp != null) append(stamp);
		append(new Gap(slot - stampWidth));
		append(body);
	}

	public static final class Gap extends LiteralText {
		public final int width;

		private Gap(int width) {
			super("");
			this.width = width;
		}

		@Override
		public Gap copy() {
			return new Gap(width);
		}
	}
}
