package com.arcticlauncher.client.packs;

import java.io.File;
import java.io.IOException;
import java.lang.reflect.InvocationTargetException;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import java.util.Optional;

import com.arcticlauncher.client.Lang;
import com.arcticlauncher.client.Platform;
import com.arcticlauncher.client.gfx.Draw;
import com.arcticlauncher.client.gfx.Gfx;
import com.arcticlauncher.client.style.Style;
import com.arcticlauncher.client.ui.Page;

/** Optional Iris integration. The client still works when no shader loader is installed. */
public final class ShaderPacks {
	public static final String NEEDS_IRIS = "looks.shaders.needs_iris";
	private final Platform platform;
	private final Class<?> iris;
	private volatile boolean changing;
	private volatile String status = "";
	private LoadingPage loading;

	public ShaderPacks(Platform platform) {
		this(platform, findIris());
	}

	ShaderPacks(Platform platform, Class<?> iris) {
		this.platform = platform;
		this.iris = iris;
	}

	private static Class<?> findIris() {
		for (String name : new String[] {"net.irisshaders.iris.Iris", "net.coderbot.iris.Iris"}) {
			try {
				return Class.forName(name, false, ShaderPacks.class.getClassLoader());
			} catch (ClassNotFoundException | LinkageError e) {
				// Another Iris generation, or shader support is not installed.
			}
		}
		return null;
	}

	public boolean available() {
		return iris != null;
	}

	public boolean changing() {
		return changing;
	}

	public String status() {
		return status;
	}

	public void clearStatus() {
		if (!changing) {
			status = "";
		}
	}

	public File folder() {
		if (iris != null) {
			try {
				return ((Path) iris.getMethod("getShaderpacksDirectory").invoke(null)).toFile();
			} catch (ReflectiveOperationException | RuntimeException e) {
				// The standard location also works before Iris finishes starting.
			}
		}
		return new File(platform.configDir().getAbsoluteFile().getParentFile(), "shaderpacks");
	}

	public List<File> installed() {
		List<File> out = new ArrayList<File>();
		File[] files = folder().listFiles();
		if (files != null) {
			for (File file : files) {
				if ((file.isFile() && file.getName().toLowerCase(Locale.ROOT).endsWith(".zip"))
						|| (file.isDirectory() && new File(file, "shaders").isDirectory())) {
					out.add(file);
				}
			}
		}
		Collections.sort(out, Comparator.comparing(File::getName, String.CASE_INSENSITIVE_ORDER));
		return out;
	}

	/** The selected pack, or null when shaders are off. Called on the game thread. */
	public String active() {
		if (iris == null) {
			return null;
		}
		try {
			Object config = iris.getMethod("getIrisConfig").invoke(null);
			boolean enabled = (Boolean) config.getClass().getMethod("areShadersEnabled").invoke(config);
			Optional<?> name = (Optional<?>) config.getClass().getMethod("getShaderPackName").invoke(config);
			return enabled && name.isPresent() ? name.get().toString() : null;
		} catch (ReflectiveOperationException | RuntimeException e) {
			return null;
		}
	}

	/** Switch packs through Iris on the render thread; null turns shaders off. */
	public void select(final String name) {
		if (!available()) {
			status = Lang.t(NEEDS_IRIS);
			return;
		}
		if (changing) {
			return;
		}
		changing = true;
		status = name == null ? Lang.t("looks.turning_shaders_off") : Lang.t("looks.loading_2") + name + "…";
		platform.runOnGameThread(() -> {
			loading = new LoadingPage(name);
			platform.openPageOver(loading);
		});
	}

	/** Called at the start of a game frame, after the preceding frame was presented. */
	public void frame() {
		LoadingPage page = loading;
		if (page == null || !page.drawn || page.reloading) {
			return;
		}
		page.reloading = true;
		try {
			reload(page.name);
		} finally {
			changing = false;
			if (loading == page) {
				loading = null;
				platform.closePage();
			}
		}
	}

	private void reload(String name) {
		Object config = null;
		String previous = null;
		boolean wasEnabled = false;
		try {
			if (name != null && !installed().contains(new File(folder(), name))) {
				throw new IOException("that shader pack is no longer in the folder");
			}
			config = iris.getMethod("getIrisConfig").invoke(null);
			Optional<?> old = (Optional<?>) config.getClass().getMethod("getShaderPackName").invoke(config);
			previous = old.isPresent() ? old.get().toString() : null;
			wasEnabled = (Boolean) config.getClass().getMethod("areShadersEnabled").invoke(config);
			configure(config, name, name != null);
			iris.getMethod("reload").invoke(null);
			if (name != null && !((Optional<?>) iris.getMethod("getCurrentPack").invoke(null)).isPresent()) {
				throw new IOException("Iris couldn't load this pack; check its compatibility");
			}
			status = name == null ? Lang.t("looks.shaders_are_off") : name + Lang.t("looks.is_on");
		} catch (ReflectiveOperationException | IOException | RuntimeException e) {
			if (config != null) {
				try {
					configure(config, previous, wasEnabled);
					iris.getMethod("reload").invoke(null);
				} catch (ReflectiveOperationException | RuntimeException restore) {
					platform.log(true, "Couldn't restore the previous shader settings: " + restore);
				}
			}
			Throwable cause = e instanceof InvocationTargetException ? e.getCause() : e;
			status = Lang.t("looks.couldnt_change_shaders") + (cause == null ? e : cause).getMessage();
		}
	}

	private final class LoadingPage extends Page {
		private final String name;
		private boolean drawn;
		private boolean reloading;

		LoadingPage(String name) {
			this.name = name;
		}

		@Override
		protected void build() {}

		@Override
		public boolean ownBackground() {
			return true;
		}

		@Override
		public boolean closeOnEscape() {
			return false;
		}

		@Override
		protected boolean rightShift() {
			return true;
		}

		@Override
		public void render(Gfx g, int mx, int my) {
			Style s = Style.ARCTIC;
			g.fill(0, 0, width, height, s.skyTop);
			Draw.centered(g, Draw.fit(g, status, width - 16), width / 2, height / 2 - 10, s.text, false);
			String hint = Lang.t("looks.shader_packs_can_take_a_while");
			Draw.centered(g, Draw.fit(g, hint, width - 16), width / 2, height / 2 + 5, s.muted, false);
			drawn = true;
		}

		@Override
		public void removed() {
			super.removed();
			if (loading == this) {
				loading = null;
				if (!reloading) {
					status = "";
					changing = false;
				}
			}
		}
	}

	private static void configure(Object config, String name, boolean enabled) throws ReflectiveOperationException {
		config.getClass().getMethod("setShaderPackName", String.class).invoke(config, name);
		config.getClass().getMethod("setShadersEnabled", boolean.class).invoke(config, enabled);
		config.getClass().getMethod("save").invoke(config);
	}
}
