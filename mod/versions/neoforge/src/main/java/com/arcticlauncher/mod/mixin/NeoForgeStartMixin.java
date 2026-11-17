package com.arcticlauncher.mod.mixin;

import com.arcticlauncher.mod.ArcticMod;
import net.minecraft.client.Minecraft;
import net.minecraft.client.main.GameConfig;
import org.objectweb.asm.Opcodes;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
//#if MC >= 1.20.2
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;
//#else
import java.io.File;
import org.spongepowered.asm.mixin.injection.Redirect;
//#endif

/** Starts the client where Fabric Loader starts client mods: as soon as the game's folder is known. */
@Mixin(Minecraft.class)
abstract class NeoForgeStartMixin {
	//#if MC >= 1.20.2
	@Inject(method = "<init>", at = @At(value = "FIELD", target = "Lnet/minecraft/client/Minecraft;gameDirectory:Ljava/io/File;", opcode = Opcodes.PUTFIELD, shift = At.Shift.AFTER))
	private void arctic$start(GameConfig config, CallbackInfo ci) {
		ArcticMod.start();
	}
	//#else
	// 1.20.1's Mixin only injects into a constructor at its end: start as the folder is read instead.
	@Redirect(method = "<init>", at = @At(value = "FIELD", target = "Lnet/minecraft/client/main/GameConfig$FolderData;gameDirectory:Ljava/io/File;", opcode = Opcodes.GETFIELD))
	private File arctic$start(GameConfig.FolderData folders) {
		ArcticMod.start();
		return folders.gameDirectory;
	}
	//#endif
}
