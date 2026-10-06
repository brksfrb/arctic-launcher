package com.arcticlauncher.legacy.mixin;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.util.Session;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Mutable;
import org.spongepowered.asm.mixin.gen.Accessor;

/** The account Minecraft plays as, for switching accounts in game. */
@Mixin(MinecraftClient.class)
public interface MinecraftClientAccountAccess {
	@Mutable
	@Accessor("session")
	void arctic$setSession(Session session);
}
