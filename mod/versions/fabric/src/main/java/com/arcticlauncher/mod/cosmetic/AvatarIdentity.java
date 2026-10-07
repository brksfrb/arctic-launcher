package com.arcticlauncher.mod.cosmetic;

//#if MC >= 1.21.2
import java.util.UUID;

/**
 * The player's UUID and its looks, kept on its render state when the state
 * is filled in (with performance mods, on several threads at once), so
 * emotes and cosmetics don't look them up again for every model part, every
 * frame (with a thousand players on screen that adds up).
 */
public interface AvatarIdentity {
	UUID arctic$uuid();

	void arctic$setUuid(UUID uuid);

	/** Its looks (cosmetics), or null: none, or not an Arctic player. */
	com.arcticlauncher.client.looks.Look arctic$look();

	void arctic$setLook(com.arcticlauncher.client.looks.Look look);

	/** When its looks were last looked up (states kept from frame to frame needn't ask every frame). */
	long arctic$lookedAt();

	void arctic$lookedAt(long millis);
}
//#endif
