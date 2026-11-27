package com.arcticlauncher.legacy.mixin;
import com.arcticlauncher.legacy.LegacyGlassCtm;
import net.minecraft.world.chunk.Chunk;
import net.minecraft.world.chunk.ChunkCache;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.Shadow;
/** The chunks a section is meshed from are a snapshot; a column the server has not sent yet is the client's empty chunk. */
@Mixin(ChunkCache.class)
abstract class GlassLegacyChunkCacheMixin implements LegacyGlassCtm.Chunks {
	@Shadow protected int minX;
	@Shadow protected int minZ;
	@Shadow protected Chunk[][] chunks;
	@Override public boolean arctic$missing(int chunkX, int chunkZ) {
		int i = chunkX - minX, j = chunkZ - minZ;
		if (i < 0 || i >= chunks.length || j < 0 || j >= chunks[i].length) return false;
		Chunk chunk = chunks[i][j];
		return chunk == null || chunk.isEmpty();
	}
}
