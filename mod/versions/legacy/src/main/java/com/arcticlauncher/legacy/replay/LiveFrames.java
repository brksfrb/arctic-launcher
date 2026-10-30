package com.arcticlauncher.legacy.replay;

import com.arcticlauncher.client.replay.ReplayBackend;
import com.arcticlauncher.client.replay.LiveVideo;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL15;
import org.lwjgl.opengl.GL21;
import org.lwjgl.opengl.GL30;

/**
 * Downscale the finished framebuffer before crossing the GPU/CPU boundary. The pixels come back through two pixel
 * buffers, one capture late: reading straight into memory makes the game wait for the GPU to finish the whole frame.
 */
final class LiveFrames {
    private static int framebuffer, texture, width, height;
    private static ByteBuffer pixels;
    private static final int[] packs = new int[2];
    private static int next;
    private static boolean primed;
    private LiveFrames() {}

    static boolean capture(int sourceWidth, int sourceHeight, int maxHeight, ReplayBackend.FrameSink sink) {
        if (!org.lwjgl.opengl.GLContext.getCapabilities().OpenGL30) return false;
        int[] size = LiveVideo.size(sourceWidth, sourceHeight, maxHeight);
        int read = GL11.glGetInteger(GL30.GL_READ_FRAMEBUFFER_BINDING);
        int draw = GL11.glGetInteger(GL30.GL_DRAW_FRAMEBUFFER_BINDING);
        int bound = GL11.glGetInteger(GL11.GL_TEXTURE_BINDING_2D);
        int alignment = GL11.glGetInteger(GL11.GL_PACK_ALIGNMENT);
        int pack = GL11.glGetInteger(GL21.GL_PIXEL_PACK_BUFFER_BINDING);
        boolean fresh;
        try {
            if (framebuffer == 0 || width != size[0] || height != size[1]) {
                if (framebuffer != 0) { GL30.glDeleteFramebuffers(framebuffer); GL11.glDeleteTextures(texture); }
                width = size[0]; height = size[1];
                framebuffer = GL30.glGenFramebuffers(); texture = GL11.glGenTextures();
                GL11.glBindTexture(GL11.GL_TEXTURE_2D, texture);
                GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MIN_FILTER, GL11.GL_LINEAR);
                GL11.glTexParameteri(GL11.GL_TEXTURE_2D, GL11.GL_TEXTURE_MAG_FILTER, GL11.GL_LINEAR);
                GL11.glTexImage2D(GL11.GL_TEXTURE_2D, 0, GL11.GL_RGBA8, width, height, 0, GL11.GL_RGBA, GL11.GL_UNSIGNED_BYTE, (ByteBuffer) null);
                GL30.glBindFramebuffer(GL30.GL_DRAW_FRAMEBUFFER, framebuffer);
                GL30.glFramebufferTexture2D(GL30.GL_DRAW_FRAMEBUFFER, GL30.GL_COLOR_ATTACHMENT0, GL11.GL_TEXTURE_2D, texture, 0);
                if (GL30.glCheckFramebufferStatus(GL30.GL_DRAW_FRAMEBUFFER) != GL30.GL_FRAMEBUFFER_COMPLETE) {
                    GL30.glDeleteFramebuffers(framebuffer); GL11.glDeleteTextures(texture); framebuffer = 0; return false;
                }
                pixels = ByteBuffer.allocateDirect(width * height * 4).order(ByteOrder.nativeOrder());
                for (int i = 0; i < 2; i++) {
                    if (packs[i] == 0) packs[i] = GL15.glGenBuffers();
                    GL15.glBindBuffer(GL21.GL_PIXEL_PACK_BUFFER, packs[i]);
                    GL15.glBufferData(GL21.GL_PIXEL_PACK_BUFFER, (long) width * height * 4, GL15.GL_STREAM_READ);
                }
                primed = false;
            }
            GL30.glBindFramebuffer(GL30.GL_DRAW_FRAMEBUFFER, framebuffer);
            GL30.glBlitFramebuffer(0, 0, sourceWidth, sourceHeight, 0, 0, width, height, GL11.GL_COLOR_BUFFER_BIT, GL11.GL_LINEAR);
            GL30.glBindFramebuffer(GL30.GL_READ_FRAMEBUFFER, framebuffer);
            GL11.glPixelStorei(GL11.GL_PACK_ALIGNMENT, 1);
            // start this frame's read into one buffer, and take the pixels the other buffer got the capture before
            GL15.glBindBuffer(GL21.GL_PIXEL_PACK_BUFFER, packs[next]);
            GL11.glReadPixels(0, 0, width, height, GL11.GL_RGBA, GL11.GL_UNSIGNED_BYTE, 0L);
            fresh = primed;
            if (fresh) {
                GL15.glBindBuffer(GL21.GL_PIXEL_PACK_BUFFER, packs[1 - next]);
                ByteBuffer mapped = GL15.glMapBuffer(GL21.GL_PIXEL_PACK_BUFFER, GL15.GL_READ_ONLY, (long) width * height * 4, null);
                pixels.clear();
                if (mapped != null) {
                    pixels.put(mapped);
                } else {
                    fresh = false;
                }
                GL15.glUnmapBuffer(GL21.GL_PIXEL_PACK_BUFFER);
                pixels.rewind();
            }
            primed = true;
            next = 1 - next;
        } finally {
            GL15.glBindBuffer(GL21.GL_PIXEL_PACK_BUFFER, pack);
            GL30.glBindFramebuffer(GL30.GL_READ_FRAMEBUFFER, read);
            GL30.glBindFramebuffer(GL30.GL_DRAW_FRAMEBUFFER, draw);
            GL11.glBindTexture(GL11.GL_TEXTURE_2D, bound);
            GL11.glPixelStorei(GL11.GL_PACK_ALIGNMENT, alignment);
        }
        if (fresh) {
            sink.frame(width, height, pixels, true);
        }
        return true;
    }
}
