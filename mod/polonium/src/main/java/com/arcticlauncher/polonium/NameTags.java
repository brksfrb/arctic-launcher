//#if MC >= 26.2
package com.arcticlauncher.polonium;

import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Supplier;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.player.Player;
import net.minecraft.world.scores.PlayerTeam;
import net.minecraft.world.scores.TeamColor;

/**
 * Each entity's name tag text, kept while what it's made of stays the same.
 * The game rebuilds every name tag's text every frame (team prefix, name,
 * suffix and color, plus hover and click info). Its parts are objects that
 * are only replaced when they change, so comparing them by identity tells
 * exactly when the text would come out different. A text that stays the same
 * object lets {@link NameTagCache} keep its layout from frame to frame.
 *
 * Only for entities whose name comes from the game's own Entity or Player
 * code; anything else gets a fresh text, as before. Render thread only.
 */
public final class NameTags {
	private static final Map<Class<?>, Boolean> PLAIN = new ConcurrentHashMap<>();

	private NameTags() {}

	/** What the text was made from, and the text. */
	public record Kept(PlayerTeam team, Component prefix, Component suffix, Optional<TeamColor> color,
			Component customName, String profileName, Component text) {}

	/** Holder of an entity's kept text (added to Entity by a mixin). */
	public interface Holder {
		Kept polonium$nameTag();

		void polonium$nameTag(Kept kept);
	}

	/** The entity's display name: the kept one if nothing it's made of changed, else {@code build}'s. */
	public static Component displayName(Entity entity, Supplier<Component> build) {
		if (!plainNames(entity.getClass())) {
			return build.get();
		}
		PlayerTeam team = entity.getTeam();
		Component prefix = team != null ? team.getPlayerPrefix() : null;
		Component suffix = team != null ? team.getPlayerSuffix() : null;
		Optional<TeamColor> color = team != null ? team.getColor() : null;
		Component customName = entity.getCustomName();
		String profileName = entity instanceof Player player ? player.getGameProfile().name() : null;
		Holder holder = (Holder) entity;
		Kept kept = holder.polonium$nameTag();
		if (kept != null && kept.team == team && kept.prefix == prefix && kept.suffix == suffix
				&& Objects.equals(kept.color, color) && kept.customName == customName
				&& Objects.equals(kept.profileName, profileName)) {
			return kept.text;
		}
		Component text = build.get();
		holder.polonium$nameTag(new Kept(team, prefix, suffix, color, customName, profileName, text));
		return text;
	}

	/** Whether this kind of entity names itself with the game's own code (nothing overrides it). */
	private static boolean plainNames(Class<?> type) {
		return PLAIN.computeIfAbsent(type, NameTags::checkPlainNames);
	}

	private static boolean checkPlainNames(Class<?> type) {
		try {
			Class<?> display = type.getMethod("getDisplayName").getDeclaringClass();
			Class<?> name = type.getMethod("getName").getDeclaringClass();
			return (display == Entity.class || display == Player.class) && (name == Entity.class || name == Player.class);
		} catch (NoSuchMethodException e) {
			return false;
		}
	}
}
//#endif
