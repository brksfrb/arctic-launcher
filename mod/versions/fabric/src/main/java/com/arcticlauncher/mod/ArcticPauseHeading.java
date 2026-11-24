//#if MC >= 26.1
package com.arcticlauncher.mod;

import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.style.Style;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.StringWidget;
import net.minecraft.network.chat.Component;

/** The native pause heading keeps its narration with the same message. */
public final class ArcticPauseHeading extends StringWidget {
	private final String heading;
	private GfxImpl gfx;

	public static StringWidget create(int x, int y, int w, int h, Component title, Font font) {
		return new ArcticPauseHeading(x, y, w, h, title, font);
	}

	private ArcticPauseHeading(int x, int y, int w, int h, Component title, Font font) {
		super(x, y, w, h, title, font);
		heading = title.getString();
	}

	@Override
	public void extractWidgetRenderState(GuiGraphicsExtractor graphics, int mx, int my, float delta) {
		if (gfx == null) gfx = new GfxImpl(graphics);
		else gfx.bind(graphics);
		gfx.menuText(Draw.TEST_MENU_FANCY);
		Draw.centered(gfx, heading, gfx.width() / 2, getY(), Style.D4.text, true);
	}
}
//#endif
