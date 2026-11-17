package com.arcticlauncher.mod;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.feature.Badge;
import com.arcticlauncher.client.feature.PingTags;
import com.arcticlauncher.client.tiers.Tiers;
import java.util.HashMap;
import java.util.Map;
import java.util.UUID;
import net.minecraft.network.chat.Component;
import net.minecraft.world.entity.player.Player;

/**
 * What goes in front of a player's name tag: the Arctic badge, the ping and the PvP tier.
 *
 * <p>From 26.2 a name tag's text is kept from frame to frame by performance mods (Polarium) for as long as the
 * game's own parts of it stay the same, with its laid-out glyphs on the text object itself. So this is added
 * after that, to the finished text ({@link com.arcticlauncher.mod.mixin.PlayerTierMixin}), and the text made here
 * stays the same object while what it is made of does: a badge that arrives a moment after the tag was first
 * drawn is shown, and a tag that hasn't changed keeps its layout.
 */
public final class NameTagDecor {
	private static final int MAX_KEPT = 512;
	private static final Map<UUID, Kept> KEPT = new HashMap<UUID, Kept>();

	private NameTagDecor() {}

	private static final class Kept {
		final Component name;
		final String made;
		final Component text;

		Kept(Component name, String made, Component text) {
			this.name = name;
			this.made = made;
			this.text = text;
		}
	}

	/** {@code name} with the decorations in front, or {@code name} itself when there are none. Game thread. */
	public static Component decorate(Player player, Component name) {
		UUID id = player.getUUID();
		Tiers.Tag tag = ArcticClient.config().tierNametags ? ArcticClient.tiers().tag(id, player.getScoreboardName()) : null;
		String ping = player == net.minecraft.client.Minecraft.getInstance().player ? null : PingTags.prefix(id);
		boolean badge = Badge.show(false, id);
		if (tag == null && ping == null && !badge) {
			KEPT.remove(id);
			return name;
		}
		String made = (tag == null ? "" : tag.tier + "\u0001" + tag.mode + "\u0001" + tag.rgb) + "\u0002" + (ping == null ? "" : ping)
				+ "\u0002" + (badge ? "b" + Badge.style(id) + Badge.role(id) : "");
		Kept kept = KEPT.get(id);
		if (kept != null && kept.name == name && kept.made.equals(made)) {
			return kept.text;
		}
		Component text = name;
		if (tag != null) {
			text = Compat.empty().append(Compat.tierTag(tag)).append(text);
		}
		if (ping != null) {
			text = Compat.empty().append(Compat.literal(ping)).append(text);
		}
		if (badge) {
			text = Compat.empty().append(Compat.arcticBadgeWithGap(id, false)).append(text);
		}
		if (KEPT.size() >= MAX_KEPT) {
			KEPT.clear();
		}
		KEPT.put(id, new Kept(name, made, text));
		return text;
	}
}
