package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.LocalClock;

import java.util.Date;

import com.arcticlauncher.client.config.ClientConfig;

/** The time in front of a chat line, as the Chat settings spell it: "[3:07:12 PM] ". */
public final class ChatStamp {
	private ChatStamp() {}

	/** Proof scenes only: pretend it is this time, so a screenshot can show 11:11 next to 08:58. */
	public static volatile Date proofNow;

	public static String text(ClientConfig c) {
		Date fixed = proofNow;
		return text(c, fixed != null ? fixed : new Date());
	}

	public static String text(ClientConfig c, Date now) {
		String pattern = (c.chatStamp12h ? "h:mm" : "HH:mm") + (c.chatStampSeconds ? ":ss" : "")
				+ (c.chatStampAmPm ? " a" : "");
		String time = LocalClock.formatter(pattern, java.util.Locale.ENGLISH).format(now);
		return (c.chatStampBrackets ? "[" + time + "]" : time) + " ";
	}
}
