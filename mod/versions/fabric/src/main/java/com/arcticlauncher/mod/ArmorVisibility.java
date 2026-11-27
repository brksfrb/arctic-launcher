package com.arcticlauncher.mod;

import com.arcticlauncher.client.feature.ArmorHider;
import com.arcticlauncher.client.feature.ArmorHider.Piece;
import net.minecraft.world.entity.EquipmentSlot;

public interface ArmorVisibility {
	boolean arctic$isLocalPlayer();

	void arctic$setLocalPlayer(boolean local);

	static boolean hides(EquipmentSlot slot) {
		switch (slot) {
			case HEAD: return ArmorHider.hides(Piece.HELMET);
			case CHEST: return ArmorHider.hides(Piece.CHESTPLATE);
			case LEGS: return ArmorHider.hides(Piece.LEGGINGS);
			case FEET: return ArmorHider.hides(Piece.BOOTS);
			default: return false;
		}
	}
}
