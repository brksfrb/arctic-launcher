//#if MC >= 1.21.9
package com.arcticlauncher.mod;

/** Implemented by the game's debug entry list (through {@code DebugEntriesMixin}): make it re-read its entries. */
public interface DebugListRebuild {
	void arctic$rebuild();
}
//#endif
