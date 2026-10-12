//#if MC >= 1.21.9
package com.arcticlauncher.mod.mixin;

import java.util.ConcurrentModificationException;
import java.util.Map;
import java.util.UUID;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.PlayerInfo;
import org.spongepowered.asm.mixin.Final;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * A crash in the game itself (1.21.9 and later): player heads look their owner up by name on a
 * background thread, walking the list of players online while the game adds and removes them, so
 * someone joining or leaving at that moment crashed the game (ConcurrentModificationException in
 * getPlayerInfoIgnoreCase). Off the game's thread the lookup reads a copy, again if it changed meanwhile.
 */
@Mixin(ClientPacketListener.class)
abstract class PlayerInfoLookupMixin {
	/** Tries at copying the list while it keeps changing; a miss only means no head texture yet. */
	private static final int TRIES = 4;

	@Shadow
	@Final
	private Map<UUID, PlayerInfo> playerInfoMap;

	@Inject(method = "getPlayerInfoIgnoreCase", at = @At("HEAD"), cancellable = true)
	private void arctic$lookUpSafely(String name, CallbackInfoReturnable<PlayerInfo> cir) {
		if (Minecraft.getInstance().isSameThread()) {
			return;
		}
		for (int i = 0; i < TRIES; i++) {
			try {
				for (PlayerInfo info : playerInfoMap.values().toArray(new PlayerInfo[0])) {
					if (info != null && name.equalsIgnoreCase(com.arcticlauncher.mod.Compat.profileName(info.getProfile()))) {
						cir.setReturnValue(info);
						return;
					}
				}
				cir.setReturnValue(null);
				return;
			} catch (ConcurrentModificationException e) {
				// The list changed while it was copied: once more.
			}
		}
		cir.setReturnValue(null);
	}
}
//#endif
