package com.arcticlauncher.mod.mixin;

//#if MC >= 1.21.6 && MC < 26.1
import com.arcticlauncher.mod.EffectsPack;
import java.util.Arrays;
import net.minecraft.client.resources.ClientPackSource;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.RepositorySource;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/** Adds the client's effects pack to the game's resource packs (1.21.6-1.21.11). */
@Mixin(PackRepository.class)
abstract class EffectsPackMixin {
	@ModifyVariable(method = "<init>", at = @At("HEAD"), argsOnly = true)
	private static RepositorySource[] arctic$sources(RepositorySource[] sources) {
		boolean client = Arrays.stream(sources).anyMatch(s -> s instanceof ClientPackSource);
		if (!client) {
			return sources;
		}
		RepositorySource[] more = Arrays.copyOf(sources, sources.length + 1);
		more[sources.length] = new EffectsPack();
		return more;
	}
}
//#endif
