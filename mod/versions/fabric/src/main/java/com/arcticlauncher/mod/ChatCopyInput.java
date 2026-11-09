package com.arcticlauncher.mod;

import com.arcticlauncher.client.feature.ChatCopy;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.ChatComponent;
import net.minecraft.client.gui.screens.ChatScreen;
import net.minecraft.network.chat.Style;
//#if MC >= 1.21.11
import net.minecraft.client.gui.ActiveTextCollector;
import net.minecraft.client.gui.TextAlignment;
import net.minecraft.util.FormattedCharSequence;
import java.util.IdentityHashMap;
//#endif

/** Uses vanilla's chat hit-test, including the same Fancy font line metrics as rendering. */
public final class ChatCopyInput {
	private ChatCopyInput() {}

	public static boolean click(int button, double x, double y, boolean shift) {
		if (!ChatCopy.enabled() || !(Compat.screen() instanceof ChatScreen)
				|| !(button == 0 || button == 1 && shift)) {
			return false;
		}
		return ChatCopy.click(hit(x, y), button, shift);
	}

	public static Style hit(double x, double y) {
		Minecraft mc = Minecraft.getInstance();
		//#if MC >= 26.2
		ChatComponent chat = mc.gui.hud.getChat();
		//#else
		ChatComponent chat = mc.gui.getChat();
		//#endif
		//#if MC >= 1.21.11
		Finder finder = new Finder(mc, (int) x, (int) y);
		//#if MC >= 26.1
		//#if MC >= 26.2
		int ticks = mc.gui.hud.getGuiTicks();
		//#else
		int ticks = mc.gui.getGuiTicks();
		//#endif
		chat.captureClickableText(finder, mc.getWindow().getGuiScaledHeight(), ticks,
				((com.arcticlauncher.mod.mixin.ChatCopyScreenAccess) Compat.screen()).arctic$displayMode());
		//#else
		chat.captureClickableText(finder, mc.getWindow().getGuiScaledHeight(), mc.gui.getGuiTicks(), true);
		//#endif
		Style style = finder.original();
		//#elif MC >= 1.16
		Style style = chat.getClickedComponentStyleAt(x, y);
		//#else
		net.minecraft.network.chat.Component part = chat.getClickedComponentAt(x, y);
		Style style = part == null ? null : part.getStyle();
		//#endif
		return style;
	}

	//#if MC >= 1.21.11
	/** Include ordinary text in this private hit-test; leave the actual insertion/click events alone. */
	private static final class Finder extends ActiveTextCollector.ClickableStyleFinder {
		private final IdentityHashMap<Style, Style> originals = new IdentityHashMap<>();
		private final IdentityHashMap<Style, Style> probes = new IdentityHashMap<>();

		Finder(Minecraft mc, int x, int y) {
			super(mc.font, x, y);
			includeInsertions(true);
		}

		@Override
		public void accept(TextAlignment alignment, int x, int y, ActiveTextCollector.Parameters parameters,
				FormattedCharSequence text) {
			super.accept(alignment, x, y, parameters, sink -> text.accept((index, style, codepoint) -> {
				Style probe = probes.get(style);
				if (probe == null) {
					probe = style.withInsertion("arctic:copy-probe");
					probes.put(style, probe);
					originals.put(probe, style);
				}
				return sink.accept(index, probe, codepoint);
			}));
		}

		Style original() { return originals.get(result()); }
	}
	//#endif
}
