package com.arcticlauncher.mod;

/** Implemented by the badge glyph mixin so a cached quad can be resized without a new allocation. */
public interface BadgeGlyphBounds {
	void arctic$setSide(float side, boolean tab);
}
