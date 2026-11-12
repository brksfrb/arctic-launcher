package com.arcticlauncher.client.feature;

import com.arcticlauncher.client.LocalClock;

import java.util.Date;

import com.arcticlauncher.client.config.ClientConfig;

/** The time in front of a chat line, as the Chat settings spell it: "[3:07:12 PM] ". */
public final class ChatStamp {
	private ChatStamp() {}

	public static String text(ClientConfig c) {
		return text(c, new Date());
	}

	public static String text(ClientConfig c, Date now) {
		String pattern = (c.chatStamp12h ? "h:mm" : "HH:mm") + (c.chatStampSeconds ? ":ss" : "")
				+ (c.chatStampAmPm ? " a" : "");
		String time = LocalClock.formatter(pattern, java.util.Locale.ENGLISH).format(now);
		return (c.chatStampBrackets ? "[" + time + "]" : time) + " ";
	}
}
