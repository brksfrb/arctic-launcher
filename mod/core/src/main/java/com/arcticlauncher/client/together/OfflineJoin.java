package com.arcticlauncher.client.together;

import java.net.InetAddress;
import java.nio.charset.StandardCharsets;
import java.util.Collection;
import java.util.UUID;

/**
 * Friends who play offline joining a Microsoft-signed-in host through Play Together, an invite or a code.
 * Those joins reach the host's world from this PC (the launcher's tunnel connects to it), so a loopback
 * connection tells them apart from people who found the world on the Wi-Fi: those keep the normal check.
 * Nothing here turns the world's sign-in check off for everyone.
 */
public final class OfflineJoin {
	/** What the host's world does with a joiner whose sign-in did not verify. */
	public enum Verdict {
		/** Not for this feature: the game decides, as it always did. */
		LEAVE,
		/** A friend through the tunnel: let them in as an offline player. */
		ACCEPT_OFFLINE,
		/** The name belongs to the host or someone in the world already. */
		SAME_NAME,
		/** The host turned the setting off: do not let this offline friend in. */
		DISABLED
	}

	private OfflineJoin() {}

	/** Whether a connection comes from this PC. */
	public static boolean loopback(InetAddress address) {
		return address != null && address.isLoopbackAddress();
	}

	/** The UUID the game's offline mode gives a name. */
	public static UUID offlineId(String name) {
		return UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes(StandardCharsets.UTF_8));
	}

	/**
	 * True once the integrated server has already given this joiner the offline UUID (vanilla does that
	 * after a failed session check). Looking at {@code GameProfile#isComplete()} is not enough: that
	 * becomes true after the UUID is filled in.
	 */
	public static boolean isOfflineProfile(String name, UUID id) {
		return name != null && id != null && id.equals(offlineId(name));
	}

	/**
	 * The host's world, for a joiner whose Microsoft session {@code verified} or not. A verified one is never
	 * touched (they keep their own profile and UUID).
	 */
	public static Verdict decide(boolean enabled, boolean integrated, boolean loopback, boolean verified, String name,
			String hostName, Collection<String> online) {
		if (!integrated || !loopback || verified) {
			return Verdict.LEAVE;
		}
		if (!enabled) {
			return Verdict.DISABLED;
		}
		if (name == null || taken(name, hostName) || online != null && online.stream().anyMatch(n -> taken(name, n))) {
			return Verdict.SAME_NAME;
		}
		return Verdict.ACCEPT_OFFLINE;
	}

	private static boolean taken(String name, String other) {
		return other != null && name.equalsIgnoreCase(other);
	}

	/**
	 * The joining side: an offline account (the launcher's token "0") going to a server on this PC, which is
	 * where Play Together's tunnel ends, skips the Microsoft sign-in call. It can only fail, and each try
	 * asks Mojang's session servers again. A real remote server still gets the call.
	 */
	public static boolean skipJoinAuth(String accessToken, boolean loopback) {
		return "0".equals(accessToken) && loopback;
	}
}
