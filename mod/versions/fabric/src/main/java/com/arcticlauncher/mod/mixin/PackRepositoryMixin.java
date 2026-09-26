package com.arcticlauncher.mod.mixin;

//#if MC >= 26.3
import com.arcticlauncher.mod.ArcticPacks;
import java.util.Arrays;
import net.minecraft.client.resources.ClientPackSource;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.packs.repository.RepositorySource;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.ModifyVariable;

/** Adds Arctic's built-in packs to the game's resource packs (not data packs). */
@Mixin(PackRepository.class)
abstract class PackRepositoryMixin {
	@ModifyVariable(method = "<init>", at = @At("HEAD"), argsOnly = true)
	private static RepositorySource[] arctic$sources(RepositorySource[] sources) {
		boolean client = Arrays.stream(sources).anyMatch(s -> s instanceof ClientPackSource);
		if (!client) {
			return sources;
		}
		RepositorySource[] more = Arrays.copyOf(sources, sources.length + 1);
		more[sources.length] = new ArcticPacks();
		return more;
	}
}
//#endif
