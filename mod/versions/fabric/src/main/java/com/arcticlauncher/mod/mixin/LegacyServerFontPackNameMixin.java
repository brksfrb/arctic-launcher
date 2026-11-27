package com.arcticlauncher.mod.mixin;

//#if MC < 1.19.3
import com.arcticlauncher.mod.ServerPackFonts;
import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
//#if MC < 1.16
import net.minecraft.server.packs.AbstractResourcePack;
import net.minecraft.server.packs.Pack;
//#else
import net.minecraft.server.packs.AbstractPackResources;
import net.minecraft.server.packs.PackResources;
//#endif
import org.spongepowered.asm.mixin.Mixin;

//#if MC < 1.16
@Mixin(AbstractResourcePack.class)
//#else
@Mixin(AbstractPackResources.class)
//#endif
abstract class LegacyServerFontPackNameMixin {
	@WrapMethod(method = "getName")
	private String arctic$keepPackProvenance(Operation<String> original) {
		//#if MC < 1.16
		return ServerPackFonts.resourcePackName((Pack) (Object) this, original.call());
		//#else
		return ServerPackFonts.resourcePackName((PackResources) (Object) this, original.call());
		//#endif
	}
}
//#endif
