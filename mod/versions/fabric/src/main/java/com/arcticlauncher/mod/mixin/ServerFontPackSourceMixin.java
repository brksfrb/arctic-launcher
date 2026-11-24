package com.arcticlauncher.mod.mixin;

//#if MC < 1.20.5
import com.arcticlauncher.mod.ServerPackFonts;
import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
//#if MC >= 1.16
import net.minecraft.server.packs.PackResources;
import net.minecraft.server.packs.repository.PackSource;
//#endif
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;

@Mixin(net.minecraft.server.packs.repository.Pack.class)
abstract class ServerFontPackSourceMixin {
	//#if MC < 1.16
	@Shadow public abstract String getId();

	@WrapMethod(method = "open")
	private net.minecraft.server.packs.Pack arctic$markServerPack(Operation<net.minecraft.server.packs.Pack> original) {
		net.minecraft.server.packs.Pack pack = original.call();
		// Old clients use the same repository ID for a world's resources.zip and a download.
		// ClientPackSource names downloaded archives with the SHA-1 of their URL.
		if (ServerPackFonts.BLOCK && "server".equals(getId()) && pack.getName().matches("[0-9a-f]{40}")) {
			ServerPackFonts.markServerPack(pack);
		}
		return pack;
	}
	//#else
	@Shadow public abstract PackSource getPackSource();

	@WrapMethod(method = "open")
	private PackResources arctic$markServerPack(Operation<PackResources> original) {
		PackResources pack = original.call();
		if (ServerPackFonts.BLOCK && getPackSource() == PackSource.SERVER) {
			ServerPackFonts.markServerPack(pack);
		}
		return pack;
	}
	//#endif
}
//#endif
