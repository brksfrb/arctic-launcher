package com.arcticlauncher.client.packs;

/** Fabric's internal resource packs must stay out of the user's pack picker. */
public final class PackVisibility {

	private PackVisibility() {}

	/**
	 * Fabric adds this flag to Minecraft's pack objects. Resource Loader v1 uses '$',
	 * older v0 releases use '_'. Optional reflection keeps Arctic usable without
	 * Fabric API and preserves genuinely selectable built-in packs such as Programmer Art.
	 */
	public static boolean isHidden(Object pack) {
		for (String hook : new String[] {"fabric$isHidden", "fabric_isHidden"}) {
			try {
				return Boolean.TRUE.equals(pack.getClass().getMethod(hook).invoke(pack));
			} catch (NoSuchMethodException e) {
				// Try the other Resource Loader generation.
			} catch (ReflectiveOperationException | SecurityException e) {
				return false;
			}
		}
		return false;
	}
}
