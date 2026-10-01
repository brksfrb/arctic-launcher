package com.arcticlauncher.mod.cosmetic;

//#if MC >= 26.1
import java.util.UUID;

/**
 * The player's UUID, kept on its render state when the state is filled in,
 * so emotes and cosmetics don't look the entity up again for every model
 * part, every frame (with a thousand players on screen that adds up).
 */
public interface AvatarIdentity {
	UUID arctic$uuid();

	void arctic$setUuid(UUID uuid);
}
//#endif
