package net.fabricmc.api;

/** Stand-in for Fabric's entry point interface, so the Arctic Client's classes load on Forge. */
public interface ClientModInitializer {
	void onInitializeClient();
}
