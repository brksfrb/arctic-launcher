package com.arcticlauncher.legacy.mixin;

import java.util.Map;
import net.minecraft.network.NetworkState;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Accessor;

/** Each phase's packet classes by id (for naming recorded packets). */
@Mixin(NetworkState.class)
public interface NetworkStateAccess {
	@Accessor("packetClasses")
	Map<?, ?> arctic$packetClasses();
}
