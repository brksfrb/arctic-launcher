package com.arcticlauncher.mod.mixin;

//#if MC >= 26.3
import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.mod.GameInfo;
import net.minecraft.client.multiplayer.MultiPlayerGameMode;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.player.Player;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Your hits, for the reach, combo and target widgets. */
@Mixin(MultiPlayerGameMode.class)
abstract class MultiPlayerGameModeMixin {
	@Inject(method = "attack", at = @At("HEAD"))
	private void arctic$attack(Player player, Entity target, CallbackInfo ci) {
		ArcticClient.features().combat().attacked(GameInfo.attacked(target));
	}
}
//#endif
