package com.arcticlauncher.client.menu;

import java.util.function.BooleanSupplier;
import java.util.function.Consumer;
import java.util.function.Supplier;

import com.arcticlauncher.client.ArcticClient;
import com.arcticlauncher.client.hud.HudWidget;

import com.arcticlauncher.client.Lang;
/**
 * One card on the Mods screen: a feature with an icon, an on/off switch
 * (when it has one) and settings, either a form ({@link #settings}) or a
 * tab of its own ({@link #page}).
 */
final class Mod {
	/** The Mods screen's filter chips. */
	enum Category {
		ALL(Lang.t("mods.all")),
		HUD("HUD"),
		VISUAL(Lang.t("mods.visual")),
		GAMEPLAY(Lang.t("mods.gameplay")),
		CHAT(Lang.t("mods.chat")),
		SOCIAL(Lang.t("mods.social")),
		CLIENT(Lang.t("mods.client"));

		final String title;

		Category(String title) {
			this.title = title;
		}
	}

	/** Builds a mod's settings into a page's form. */
	interface Settings {
		void build(Host host, Form f);
	}

	final String id;
	final String name;
	final String description;
	/** Icon file in assets/arctic/icons (without .png). */
	final String icon;
	final Category category;
	/** Extra words the search matches. */
	final String keywords;
	private BooleanSupplier on;
	private Consumer<Boolean> set;
	Settings settings;
	/** A whole tab of its own instead of a settings form. */
	Supplier<MenuTab> page;
	/** For HUD widgets: the widget (its page shows a live preview). */
	HudWidget widget;

	Mod(String id, String name, String description, String icon, Category category, String keywords) {
		this.id = id;
		this.name = name;
		this.description = description;
		this.icon = icon;
		this.category = category;
		this.keywords = keywords == null ? "" : keywords;
	}

	/** Give it an on/off switch (saved on change). */
	Mod toggle(BooleanSupplier get, Consumer<Boolean> set) {
		this.on = get;
		this.set = set;
		return this;
	}

	Mod settings(Settings s) {
		this.settings = s;
		return this;
	}

	Mod page(Supplier<MenuTab> p) {
		this.page = p;
		return this;
	}

	boolean hasSwitch() {
		return on != null;
	}

	boolean on() {
		return on != null && on.getAsBoolean();
	}

	void setOn(boolean value) {
		if (set != null) {
			set.accept(value);
			ArcticClient.saveConfig();
		}
	}

	boolean hasSettings() {
		return settings != null || page != null;
	}

	/** Does the search text match (name, description or keywords)? */
	boolean matches(String query) {
		if (query == null || query.trim().isEmpty()) {
			return true;
		}
		String q = query.trim().toLowerCase(java.util.Locale.ROOT);
		return (name + " " + description + " " + keywords).toLowerCase(java.util.Locale.ROOT).contains(q);
	}

	/** The icon's texture key for {@code Gfx.texture}. */
	String iconKey() {
		return "asset:icons/" + icon;
	}
}
