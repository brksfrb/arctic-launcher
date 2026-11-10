package com.arcticlauncher.mod.mixin;

//#if MC >= 1.16
import net.minecraft.network.chat.Style;
import net.minecraft.network.chat.TextColor;
import net.minecraft.network.chat.ClickEvent;
import net.minecraft.network.chat.HoverEvent;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.gen.Invoker;

@Mixin(Style.class)
interface ChatCopyStyleAccess {
	@Invoker("<init>")
	static Style arctic$empty(TextColor color,
			//#if MC >= 1.21.4
			Integer shadow,
			//#endif
			Boolean bold, Boolean italic, Boolean underlined, Boolean strike, Boolean obfuscated,
			ClickEvent click, HoverEvent hover, String insertion,
			//#if MC >= 1.21.9
			net.minecraft.network.chat.FontDescription font
			//#else
			net.minecraft.resources.ResourceLocation font
			//#endif
	) { throw new AssertionError(); }
}
//#endif
