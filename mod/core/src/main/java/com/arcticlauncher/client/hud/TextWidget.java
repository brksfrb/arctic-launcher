package com.arcticlauncher.client.hud;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.config.HudSlot;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;

/**
 * A label and a value in a box: "FPS: 144", "144 FPS" or "[FPS: 144]", as
 * the player set it, or an icon in place of the label (the mod's icon for it, or the item it is about).
 * Boxes share one minimum width, so a column of them lines up; the text sits in the middle.
 */
public abstract class TextWidget extends HudWidget {
	private static final int PAD = 5;
	private static final int HEIGHT = 16;
	/** Boxes are at least this wide, so stacked ones line up. */
	private static final int MIN_WIDTH = 58;
	/** An icon drawn from the mod's icon sheet, and the room it takes (with the gap to the value). */
	private static final int ICON = 10;
	private static final int ICON_ROOM = 14;
	/** An item's picture is 16 pixels. */
	private static final int ITEM_ROOM = 19;

	private final String label;
	/** The icon for this widget (a name in the mod's icons), or null for none. */
	private String iconName;
	/** Items whose picture stands for this widget (the first the game has), or null. */
	private String[] iconItems;
	/** Starts with the icon instead of the label. */
	private boolean iconByDefault;
	/** The widget writes its own labels in the value (like "X: 4  Y: 70"): no label, whatever the format. */
	private boolean ownLabels;
	/** The item picked for the icon (looked up once). */
	private Object itemStack;
	private boolean itemLooked;
	/** Whether the last draw showed the sample value (the editor), so the width matches it. */
	private boolean previewing;
	/** This HUD draw's text (label part, value part, order), worked out once per draw. */
	private String[] current;
	private long currentFrame = -1;
	private boolean currentPreview;
	private HudSlot currentLook;
	/** The text last measured, and its parts' widths (measured again only when it changes). */
	private String[] measuredParts;
	private int partWidth0;
	private int partWidth1;
	/** The font the widths were measured in (smooth or the game's). */
	private boolean measuredFancy;

	protected TextWidget(String id, String label, String description, boolean onByDefault) {
		super(id, label, description, onByDefault, Column.LEFT);
		this.label = label;
	}

	protected abstract String value(boolean preview);

	/** The mod's icon {@code name} stands for this widget (an icon the sheet has); {@code first}: it is shown from the start. */
	public TextWidget icon(String name, boolean first) {
		this.iconName = name;
		this.iconByDefault = first;
		return this;
	}

	/** An item's picture stands for this widget: the first of these the game has. Shown from the start. */
	public TextWidget itemIcon(String... items) {
		this.iconItems = items;
		this.iconByDefault = true;
		return this;
	}

	/** The widget's value carries its own labels: it is drawn without a label. */
	public TextWidget ownLabels() {
		this.ownLabels = true;
		return this;
	}

	/** Which icon a widget gets without being told: the sheet's icon with its own name, when there is one. */
	private String iconNameOrDefault() {
		if (iconName != null) {
			return iconName;
		}
		return TextWidget.class.getResource("/assets/arctic/icons/" + id + ".png") != null ? id : null;
	}

	/** The widget has a label to pick a format for (not where the value carries its own labels). */
	public boolean hasLabel() {
		return !ownLabels;
	}

	/** Has an icon to show in place of the label. */
	public boolean hasIcon() {
		return !ownLabels && (iconItems != null || iconNameOrDefault() != null);
	}

	/** Starts with the icon. */
	public boolean iconFirst() {
		return iconByDefault && hasIcon();
	}

	/** The sample value, as the editor shows it. */
	public String sample() {
		return value(true);
	}

	/** The label formats the player can pick (their {@link HudSlot} label modes follow in {@link #labelModes()}). */
	public String[] labelChoices() {
		String sample = sample();
		java.util.List<String> out = new java.util.ArrayList<String>();
		if (hasIcon()) {
			out.add("Icon");
		}
		out.add(label + ": " + sample);
		out.add(sample + " " + label);
		out.add(sample);
		return out.toArray(new String[0]);
	}

	public int[] labelModes() {
		return hasIcon()
				? new int[] {HudSlot.LABEL_ICON, HudSlot.LABEL_BEFORE, HudSlot.LABEL_AFTER, HudSlot.LABEL_NONE}
				: new int[] {HudSlot.LABEL_BEFORE, HudSlot.LABEL_AFTER, HudSlot.LABEL_NONE};
	}

	private boolean iconMode() {
		return look.labelMode == HudSlot.LABEL_ICON && hasIcon();
	}

	/** The label part (colored with the label color), then the value part. */
	private String[] parts(String value) {
		String open = look.brackets ? "[" : "";
		String close = look.brackets ? "]" : "";
		if (ownLabels) {
			return new String[] {"", open + value + close, "none"};
		}
		if (iconMode()) {
			return new String[] {"", open + value + close, "icon"};
		}
		switch (look.labelMode) {
			case HudSlot.LABEL_AFTER:
				return new String[] {open + value + " ", label + close, "after"};
			case HudSlot.LABEL_NONE:
				return new String[] {"", open + value + close, "none"};
			default:
				return new String[] {open + label + ": ", value + close, "before"};
		}
	}

	/** This HUD draw's text: the value is asked for once per draw. */
	private String[] current(boolean preview) {
		if (current == null || currentFrame != Hud.frame() || currentPreview != preview || currentLook != look) {
			current = parts(value(preview));
			currentFrame = Hud.frame();
			currentPreview = preview;
			currentLook = look;
		}
		return current;
	}

	@Override
	public Object content(boolean preview) {
		if (look.chroma) {
			return null;
		}
		String[] p = current(preview);
		return p[0] + '' + p[1] + '' + p[2];
	}

	/** This draw's text, measured (only the parts that changed since the last measuring). */
	private String[] measured(Gfx g, boolean preview) {
		String[] p = current(preview);
		boolean fancy = com.arcticlauncher.client.gfx.Draw.fancy;
		boolean fontChanged = measuredFancy != fancy;
		if (measuredParts == null || fontChanged || !p[0].equals(measuredParts[0])) {
			partWidth0 = g.textWidth(p[0]);
		}
		if (measuredParts == null || fontChanged || !p[1].equals(measuredParts[1])) {
			partWidth1 = g.textWidth(p[1]);
		}
		measuredParts = p;
		measuredFancy = fancy;
		return p;
	}

	/** The room the icon takes in front of the value (0 without one). */
	private int iconRoom() {
		if (!iconMode()) {
			return 0;
		}
		return iconItems != null ? ITEM_ROOM : ICON_ROOM;
	}

	private int textWidth(Gfx g) {
		measured(g, previewing);
		return partWidth0 + partWidth1 + iconRoom();
	}

	@Override
	public int width(Gfx g) {
		int text = textWidth(g);
		return background() ? Math.max(MIN_WIDTH, text + PAD * 2) : text;
	}

	@Override
	public int height() {
		return HEIGHT;
	}

	private Object item() {
		if (!itemLooked && ArcticClient.platform() != null) {
			itemLooked = true;
			for (String id : iconItems) {
				itemStack = ArcticClient.platform().sampleItem(id);
				if (itemStack != null) {
					break;
				}
			}
		}
		return itemStack;
	}

	@Override
	public void render(Gfx g, Style s, boolean preview) {
		previewing = preview;
		String[] p = measured(g, preview);
		int w = width(g);
		panel(g, s, 0, 0, w, HEIGHT);
		int x = (w - textWidth(g)) / 2;
		int y = (HEIGHT - 8) / 2;
		if (iconMode()) {
			if (iconItems != null) {
				Object stack = item();
				if (stack != null) {
					g.itemIcon(stack, x, 0);
				}
			} else {
				g.texture("asset:icons/" + iconNameOrDefault(), x, (HEIGHT - ICON) / 2, ICON, ICON, 0, 0, 64, 64, 64, 64);
			}
			x += iconRoom();
		}
		boolean labelFirst = !"after".equals(p[2]);
		// The label keeps its own color; after the value, "FPS" is the label.
		int first = labelFirst ? accent() : text();
		int second = labelFirst ? text() : accent();
		if (ownLabels && p[0].isEmpty()) {
			drawOwnLabels(g, p[1], x, y);
			return;
		}
		g.text(p[0], x, y, first, shadow());
		g.text(p[1], x + partWidth0, y, second, shadow());
	}

	/** A value with its own labels ("X: 4  Y: 70  Z: 3"): each word ending in a colon is a label, in the accent colour. */
	private void drawOwnLabels(Gfx g, String text, int x, int y) {
		int at = x;
		int i = 0;
		while (i < text.length()) {
			int word = i;
			while (word < text.length() && text.charAt(word) != ' ') {
				word++;
			}
			int end = word;
			while (end < text.length() && text.charAt(end) == ' ') {
				end++;
			}
			String token = text.substring(i, end);
			boolean label = word > i && text.charAt(word - 1) == ':';
			g.text(token, at, y, label ? accent() : text(), shadow());
			at += g.textWidth(token);
			i = end;
		}
	}
}
