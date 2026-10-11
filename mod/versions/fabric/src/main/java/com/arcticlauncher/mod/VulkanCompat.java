package com.arcticlauncher.mod;

import net.fabricmc.loader.api.FabricLoader;

/** VulkanMod (Vulkan instead of OpenGL): what the client does differently under it. */
public final class VulkanCompat {
	/** Whether VulkanMod renders the game. */
	public static final boolean ACTIVE = FabricLoader.getInstance().isModLoaded("vulkanmod");

	private VulkanCompat() {}

	/**
	 * 1.21.6-1.21.11 blur with the game's post effect (MotionBlurPostMixin) instead of OpenGL calls
	 * (MotionBlurGL): under VulkanMod, or when asked to (-Darctic.blurPost, to check that path).
	 */
	public static boolean postBlur() {
		//#if MC >= 1.21.6
		return ACTIVE || Boolean.getBoolean("arctic.blurPost");
		//#else
		return false;
		//#endif
	}
}
