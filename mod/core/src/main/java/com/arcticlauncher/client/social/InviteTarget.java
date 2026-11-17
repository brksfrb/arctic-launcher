package com.arcticlauncher.client.social;

import java.util.function.Predicate;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;

/** The two supported invite destinations; unknown or incomplete invites have no action. */
final class InviteTarget {
	private final boolean together;
	private final String target;

	private InviteTarget(boolean together, String target) {
		this.together = together;
		this.target = target;
	}

	static InviteTarget parse(JsonObject invite) {
		String kind = text(invite, "kind");
		String target = text(invite, "target").trim();
		if (text(invite, "id").isEmpty() || target.isEmpty()
				|| !("together".equals(kind) || "server".equals(kind))) {
			return null;
		}
		return new InviteTarget("together".equals(kind), target);
	}

	boolean join(Predicate<String> joinTogether, Predicate<String> joinServer) {
		return (together ? joinTogether : joinServer).test(target);
	}

	private static String text(JsonObject object, String key) {
		JsonElement value = object == null ? null : object.get(key);
		return value != null && value.isJsonPrimitive() && value.getAsJsonPrimitive().isString()
				? value.getAsString() : "";
	}
}
