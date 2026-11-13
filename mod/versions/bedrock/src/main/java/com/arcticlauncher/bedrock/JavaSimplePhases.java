package com.arcticlauncher.bedrock;

import java.util.ArrayList;
import java.util.Collections;
import java.util.HashMap;
import java.util.Iterator;
import java.util.List;
import java.util.Map;

/** SimpleFeatureRenderPhase for the owned avatar collector, with debug shuffle
 * disabled. These are this renderer's object/hash/cache histories, not captured
 * Java game hashes or a lease on Bedrock's world phase. Skin model distance
 * ordering and foreign feature submissions remain separate owning inputs. */
final class JavaSimplePhases {
    /** Owned submission identity, shared by packet staging and software
     * collection. A model token is compared by reference, never by pose. */
    interface Submit {
        NativeAvatarGeometry.Pass phasePass();
        JavaRenderType phaseType();
        Object phaseModel();
    }
    static final class Phase {
        final int order,stage;
        private final int[] batches;
        Phase(int order,int stage,List<Integer> indices) {
            this.order=order;this.stage=stage;batches=new int[indices.size()];
            for(int i=0;i<batches.length;i++)batches[i]=indices.get(i);
        }
        int[] batches(){return batches.clone();}
    }
    static final class Result {
        final long epoch;
        final List<Phase> phases;
        private final WorldFrame world;
        private final Object geometry;
        Result(long epoch,WorldFrame world,Object geometry,List<Phase> phases) {
            this.epoch=epoch;this.world=world;this.geometry=geometry;
            this.phases=Collections.unmodifiableList(new ArrayList<Phase>(phases));
        }
        void requireSource(WorldFrame world,Object geometry) {
            if(this.world!=world||this.geometry!=geometry)throw new IllegalArgumentException("simple phases belong to different staging input");
        }
    }
    private static final class Feature {
        // RenderType has reference equality/hashCode. Keep the actual owned
        // JavaRenderType objects as keys, rather than their numeric wire IDs.
        private final Map<JavaRenderType,List<List<Integer>>> batches=new HashMap<JavaRenderType,List<List<Integer>>>();
        void add(JavaRenderType type,List<Integer> submit) {
            batches.computeIfAbsent(type,key->new ArrayList<List<Integer>>()).add(submit);
        }
        Phase drain(int order,int stage) {
            List<Integer> indices=new ArrayList<Integer>();
            for(List<List<Integer>> batch:batches.values())for(List<Integer> submit:batch)indices.addAll(submit);
            if(indices.isEmpty())return null;
            // FeatureSubmits.clear removes lists which were already empty,
            // and clears/retains lists used in this drain. It is not map.clear.
            Iterator<List<List<Integer>>> iterator=batches.values().iterator();
            while(iterator.hasNext()) {
                List<List<Integer>> submits=iterator.next();
                if(submits.isEmpty())iterator.remove();else submits.clear();
            }
            return new Phase(order,stage,indices);
        }
    }
    private static final class Order {
        final Feature solid=new Feature(),custom=new Feature();
    }
    private String session,dimension;
    private long epoch;
    private Order ordinary,emissive;

    private void invalidate() {session=null;dimension=null;ordinary=null;emissive=null;}
    synchronized void reset() {invalidate();}

    /** One serialized geometry collection/drain, including empty collections.
     * A gap/unsupported input invalidates the owned history. Later valid input
     * starts a new explicit epoch instead of claiming continuous history. */
    synchronized Result collect(WorldFrame world,NativeAvatarGeometry geometry) {
        return collect(world,geometry,geometry.batches());
    }
    synchronized Result collect(WorldFrame world,Object source,List<? extends Submit> batches) {
        if(world==null||source==null||batches==null||batches.size()>2048) {
            invalidate();throw new IllegalArgumentException("invalid owned simple-phase collection");
        }
        java.util.Set<Object> models=Collections.newSetFromMap(new java.util.IdentityHashMap<Object,Boolean>());
        Object previousModel=null;NativeAvatarGeometry.Pass previousPass=null;JavaRenderType previousType=null;
        for(Submit batch:batches) {
            NativeAvatarGeometry.Pass pass=batch.phasePass();JavaRenderType type=batch.phaseType();Object model=batch.phaseModel();
            if(pass==null||pass==NativeAvatarGeometry.Pass.GENERAL||type==null
                    ||(pass==NativeAvatarGeometry.Pass.CAPE_SOLID||pass==NativeAvatarGeometry.Pass.WING_CUTOUT)&&model==null
                    ||model!=null&&(model==previousModel?(pass!=previousPass||type!=previousType):!models.add(model))) {
                invalidate();return null;
            }
            previousModel=model;previousPass=pass;previousType=type;
        }
        if(!world.session.equals(session)||!world.dimension.equals(dimension)) {
            invalidate();
            if(epoch==Long.MAX_VALUE)throw new IllegalArgumentException("simple phase epoch exhausted");
            epoch++;session=world.session;dimension=world.dimension;
        }
        boolean ordinaryUsed=false,emissiveUsed=false;
        for(int index=0;index<batches.size();) {
            Submit batch=batches.get(index);
            List<Integer> submit=new ArrayList<Integer>();submit.add(index++);
            if(batch.phaseModel()!=null) {
                while(index<batches.size()&&batches.get(index).phaseModel()==batch.phaseModel())submit.add(index++);
            }
            if(batch.phasePass()==NativeAvatarGeometry.Pass.COSMETIC_GLOW) {
                emissiveUsed=true;if(emissive==null)emissive=new Order();
                emissive.custom.add(batch.phaseType(),submit);
            } else {
                ordinaryUsed=true;if(ordinary==null)ordinary=new Order();
                switch(batch.phasePass()) {
                    case CAPE_SOLID:case WING_CUTOUT:ordinary.solid.add(batch.phaseType(),submit);break;
                    case COSMETIC_BASE:case BENT_SKIN:ordinary.custom.add(batch.phaseType(),submit);break;
                    // This phase participates in order lifetime, but its
                    // distinct unstable model sorter runs on native inputs.
                    case SKIN_TRANSLUCENT:break;
                    default:throw new IllegalArgumentException("unsupported simple phase role");
                }
            }
        }
        List<Phase> phases=new ArrayList<Phase>();
        if(ordinaryUsed) {
            add(phases,ordinary.solid.drain(0,0));
            add(phases,ordinary.custom.drain(0,8));
        } else {ordinary=null;}
        if(emissiveUsed)add(phases,emissive.custom.drain(1,8));else emissive=null;
        // Empty phases are skipped: their retained empty-key history survives
        // while another phase keeps this numeric order alive. A completely
        // empty order is removed, matching SubmitNodeStorage.drainPhases.
        return new Result(epoch,world,source,phases);
    }
    private static void add(List<Phase> phases,Phase phase) {if(phase!=null)phases.add(phase);}
}
