//#if MC >= 1.18
package com.arcticlauncher.mod.startup;

import java.util.logging.Handler;
import java.util.logging.Level;
import java.util.logging.LogRecord;
import java.util.logging.Logger;

/**
 * Prints who ends the game with {@code System.exit} or {@code Runtime.exit} (Java 21+ logs the
 * call and its stack under {@code java.lang.Runtime} at debug level). A game that exits by
 * itself with a code and no crash report otherwise leaves nothing to say why.
 */
final class ExitTrace {
	/** The logger is held weakly by the log manager: kept here so the setting stays. */
	private static Logger logger;

	private ExitTrace() {}

	static void install() {
		try {
			logger = Logger.getLogger("java.lang.Runtime");
			logger.setLevel(Level.FINE);
			logger.setUseParentHandlers(false);
			logger.addHandler(new Handler() {
				{
					setLevel(Level.ALL);
				}

				@Override
				public void publish(LogRecord record) {
					System.err.println("[arctic] " + record.getMessage());
					System.err.flush();
				}

				@Override
				public void flush() {}

				@Override
				public void close() {}
			});
		} catch (Throwable ignored) {
			// A game that exits quietly is no worse off.
		}
	}
}
//#endif
