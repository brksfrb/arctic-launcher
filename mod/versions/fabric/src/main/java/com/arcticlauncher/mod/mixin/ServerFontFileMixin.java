package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.ServerPackFonts;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import java.io.IOException;
import java.io.InputStream;
//#if MC >= 1.20
import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.mojang.blaze3d.font.GlyphProvider;
import it.unimi.dsi.fastutil.ints.IntSets;
import net.minecraft.client.gui.font.providers.TrueTypeGlyphProviderDefinition;
//#else
import net.minecraft.client.gui.font.providers.TrueTypeGlyphProviderBuilder;
//#endif
//#if MC >= 1.19
import java.util.List;
import net.minecraft.resources.Identifier;
import net.minecraft.server.packs.resources.ResourceManager;
//#endif
import net.minecraft.server.packs.resources.Resource;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

/** A server pack can also override the file named by an otherwise local TTF provider. */
//#if MC >= 1.20
@Mixin(TrueTypeGlyphProviderDefinition.class)
//#else
@Mixin(TrueTypeGlyphProviderBuilder.class)
//#endif
abstract class ServerFontFileMixin {
	//#if MC >= 1.20
	@WrapMethod(method = "load")
	private GlyphProvider arctic$skipBlockedFont(ResourceManager manager, Operation<GlyphProvider> original) {
		try {
			return original.call(manager);
		} catch (ServerPackFonts.BlockedFont blocked) {
			// A failed builder can invalidate the whole default font. Keep the other providers.
			return () -> IntSets.EMPTY_SET;
		}
	}
	//#endif

	//#if MC >= 1.19
	@WrapOperation(
			//#if MC >= 1.20
			method = "load",
			//#else
			method = "create",
			//#endif
			at = @At(value = "INVOKE",
					target = "Lnet/minecraft/server/packs/resources/ResourceManager;open(Lnet/minecraft/resources/Identifier;)Ljava/io/InputStream;"))
	private InputStream arctic$checkFontFile(ResourceManager manager, Identifier path, Operation<InputStream> original)
			throws IOException {
		if (!ServerPackFonts.BLOCK) {
			return original.call(manager, path);
		}
		Resource resource = manager.getResourceOrThrow(path);
		if (!ServerPackFonts.fromBlockedPack(resource)) {
			return resource.open();
		}
		// The resource stack lists the highest-priority pack last.
		List<Resource> stack = manager.getResourceStack(path);
		for (int i = stack.size() - 1; i >= 0; i--) {
			Resource candidate = stack.get(i);
			if (!ServerPackFonts.fromBlockedPack(candidate)) {
				return candidate.open();
			}
		}
		throw new ServerPackFonts.BlockedFont(path.toString());
	}
	//#else
	@WrapOperation(method = "create", at = @At(value = "INVOKE",
			target = "Lnet/minecraft/server/packs/resources/Resource;getInputStream()Ljava/io/InputStream;"))
	private InputStream arctic$checkFontFile(Resource resource, Operation<InputStream> original) {
		if (ServerPackFonts.fromBlockedPack(resource)) {
			throw new IllegalArgumentException("TrueType font from a server resource pack");
		}
		return original.call(resource);
	}
	//#endif
}
