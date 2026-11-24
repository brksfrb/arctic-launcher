package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.ServerPackFonts;
import com.google.gson.JsonArray;
import com.google.gson.JsonObject;
import com.llamalad7.mixinextras.injector.wrapoperation.Operation;
import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;
import com.llamalad7.mixinextras.sugar.Local;
//#if MC >= 1.20
import com.google.gson.Gson;
import com.google.gson.JsonElement;
import java.io.Reader;
import net.minecraft.client.gui.font.FontManager;
//#endif
import net.minecraft.server.packs.resources.Resource;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;

//#if MC >= 1.20
@Mixin(FontManager.class)
//#elif MC >= 1.14
@Mixin(targets = "net.minecraft.client.gui.font.FontManager$1")
//#else
@Mixin(targets = "net.minecraft.class_4130")
//#endif
abstract class ServerFontDefinitionMixin {
	//#if MC >= 1.20
	@WrapOperation(method = "loadResourceStack", at = @At(value = "INVOKE",
			target = "Lcom/google/gson/Gson;fromJson(Ljava/io/Reader;Ljava/lang/Class;)Ljava/lang/Object;"))
	private static Object arctic$filterDefinitions(Gson gson, Reader reader, Class<?> type, Operation<Object> original,
			@Local Resource source) {
		Object decoded = original.call(gson, reader, type);
		if (ServerPackFonts.fromBlockedPack(source) && decoded instanceof JsonObject object) {
			JsonElement providers = object.get("providers");
			if (providers != null && providers.isJsonArray()) {
				object.add("providers", ServerPackFonts.withoutTrueType(providers.getAsJsonArray()));
			}
		}
		return decoded;
	}
	//#else
	@WrapOperation(
			//#if MC >= 1.14
			method = "prepare",
			//#else
			method = "onResourceManagerReload",
			//#endif
			at = @At(value = "INVOKE",
					target = "Lnet/minecraft/util/GsonHelper;getAsJsonArray(Lcom/google/gson/JsonObject;Ljava/lang/String;)Lcom/google/gson/JsonArray;"))
	private JsonArray arctic$filterDefinitions(JsonObject json, String key, Operation<JsonArray> original,
			@Local Resource source) {
		JsonArray providers = original.call(json, key);
		return ServerPackFonts.fromBlockedPack(source) ? ServerPackFonts.withoutTrueType(providers) : providers;
	}
	//#endif
}
