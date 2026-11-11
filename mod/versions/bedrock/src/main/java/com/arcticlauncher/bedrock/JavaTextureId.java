package com.arcticlauncher.bedrock;

/** Logical Java resource identity, independent of copied pixels/GPU handles.
 * Paths mirror Fabric GfxImpl.look and FabricPlatform asset registration. */
final class JavaTextureId {
    static final String MESH_WHITE="arctic:look/mesh/white";
    private JavaTextureId() {}
    static String look(String path){String id="arctic:look/"+path;validate(id);return id;}
    static void validate(String id) {
        if(id!=null&&(id.length()>256||!id.matches("[a-z0-9_.-]+:[a-z0-9_./-]+")))
            throw new IllegalArgumentException("invalid or oversized Java texture identifier");
    }
}
