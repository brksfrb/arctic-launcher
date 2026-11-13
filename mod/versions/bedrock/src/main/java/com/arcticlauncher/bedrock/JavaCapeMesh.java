package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.Xform;

/** Java 26.2 PlayerCapeModel's uninflated 10x16x1 box, local to the cape
 * part. Vanilla's Polygon winding and bottom-face V differ from the shared
 * cosmetic cuboid emitter; a solid, culled cape needs its own mesh. */
final class JavaCapeMesh {
    // CubeDefinition's (1,.5) texScale on the 64x64 layer yields 64x32.
    private static final JavaModelBox BOX=new JavaModelBox(-5,0,-1,10,16,1,0,0,0,64,32,false);

    private JavaCapeMesh() {}

    static void emit(Xform target) {
        BOX.emit(target);
    }

    static void emit(Xform target,int light) {
        BOX.emit(target,light);
    }
}
