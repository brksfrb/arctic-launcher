package com.arcticlauncher.legacy;

import com.arcticlauncher.client.SessionLine;
import org.apache.logging.log4j.LogManager;
import org.apache.logging.log4j.core.LogEvent;
import org.apache.logging.log4j.core.filter.AbstractFilter;
import org.apache.logging.log4j.message.Message;

/**
 * Keeps the access token out of the game's log files: the game logs "(Session ID is token:...)" as it starts
 * (into latest.log on 1.8.9 and 1.9.4, debug.log later). That line is written without the token instead.
 */
public final class SessionLogMask extends AbstractFilter {
	private static boolean installed;

	private SessionLogMask() {}

	/** Before the game's first line (its main() calls this). Without Log4j's core there is nothing to do. */
	public static synchronized void install() {
		if (installed) {
			return;
		}
		installed = true;
		try {
			((org.apache.logging.log4j.core.Logger) LogManager.getRootLogger()).addFilter(new SessionLogMask());
		} catch (Throwable t) {
			// Another logging back end: its files are not ours to change.
		}
	}

	@Override
	public Result filter(LogEvent event) {
		Message message = event.getMessage();
		String text = message == null ? null : message.getFormattedMessage();
		if (!SessionLine.secret(text)) {
			return Result.NEUTRAL;
		}
		// The masked line no longer counts as secret, so it passes this filter.
		LogManager.getLogger(event.getLoggerName()).log(event.getLevel(), SessionLine.mask(text));
		return Result.DENY;
	}
}
