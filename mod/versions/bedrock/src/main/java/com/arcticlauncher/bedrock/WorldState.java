package com.arcticlauncher.bedrock;

/** A missing render frame is separate from leaving the native world. */
final class WorldState {
    WorldFrame current;
    boolean renderable;

    boolean frame(WorldFrame next) {
        if(next==null)return gap(false);
        if(next.geometryOnly())throw new IllegalArgumentException("geometry-only input cannot become a renderable world");
        boolean changed=!java.util.Objects.equals(current==null?null:current.session,next.session);
        current=next;renderable=true;
        return changed;
    }
    boolean gap(boolean retainWorld) {
        renderable=false;
        if(retainWorld)return false;
        boolean changed=current!=null;
        current=null;
        return changed;
    }
}
