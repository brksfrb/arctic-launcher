package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.Attach;
import com.arcticlauncher.client.looks.Xform;
import java.util.*;

/** Java 26.2 HumanoidModel/PlayerModel skin cubes, local to each supplied
 * model part. This owns geometry only; it does not infer a native bone pose,
 * skin visibility setting, hurt overlay or material variant. */
final class JavaPlayerMesh {
    private static final JavaModelBox[][] WIDE=model(false),SLIM=model(true);
    private static final List<Attach> ROOT_PARTS=rootParts();
    private JavaPlayerMesh() {}

    /** PartDefinition bakes its default HashMap entry traversal into an
     * insertion-ordered Object2ObjectArrayMap. ModelPart.render walks those
     * values, not Arctic's rig/limb enumeration or a sorted name sequence. */
    static List<Attach> renderParts(){return ROOT_PARTS;}
    private static List<Attach> rootParts() {
        Map<String,Attach> children=new HashMap<String,Attach>();
        // HumanoidModel.createMesh root additions. Hat is a child of head;
        // PlayerModel replaces existing arms/legs and adds their child layers.
        children.put("head",Attach.HEAD);
        children.put("body",Attach.BODY);
        children.put("right_arm",Attach.RIGHT_ARM);
        children.put("left_arm",Attach.LEFT_ARM);
        children.put("right_leg",Attach.RIGHT_LEG);
        children.put("left_leg",Attach.LEFT_LEG);
        return Collections.unmodifiableList(new ArrayList<Attach>(children.values()));
    }

    static void emit(Attach part,boolean slim,boolean outer,Xform target,int light) {
        JavaModelBox[] boxes=(slim?SLIM:WIDE)[part.ordinal()];
        boxes[0].emit(target,light);
        // Each outer layer is a ZERO-pose child of its owning vanilla part.
        if(outer)boxes[1].emit(target,light);
    }
    private static JavaModelBox[][] model(boolean slim) {
        JavaModelBox[][] model=new JavaModelBox[Attach.values().length][];
        for(Attach part:Attach.values()) {
            float x,y,z,width,height,depth,grow;
            int u,v,outerU,outerV;
            switch(part) {
                case HEAD:
                    x=-4;y=-8;z=-4;width=height=depth=8;u=v=0;outerU=32;outerV=0;grow=.5f;break;
                case BODY:
                    x=-4;y=0;z=-2;width=8;height=12;depth=4;u=v=16;outerU=16;outerV=32;grow=.25f;break;
                case RIGHT_ARM:
                    x=slim?-2:-3;y=-2;z=-2;width=slim?3:4;height=12;depth=4;
                    u=40;v=16;outerU=40;outerV=32;grow=.25f;break;
                case LEFT_ARM:
                    x=-1;y=-2;z=-2;width=slim?3:4;height=12;depth=4;
                    u=32;v=48;outerU=48;outerV=48;grow=.25f;break;
                case RIGHT_LEG:
                    x=-2;y=0;z=-2;width=4;height=12;depth=4;u=0;v=16;outerU=0;outerV=32;grow=.25f;break;
                case LEFT_LEG:
                    x=-2;y=0;z=-2;width=4;height=12;depth=4;u=16;v=48;outerU=0;outerV=48;grow=.25f;break;
                default:throw new IllegalArgumentException("unknown player model part");
            }
            // PlayerModel replaces the inherited mirrored left arm/leg with
            // independent modern skin regions. Both sides use mirror=false.
            model[part.ordinal()]=new JavaModelBox[]{
                new JavaModelBox(x,y,z,width,height,depth,0,u,v,64,64,false),
                new JavaModelBox(x,y,z,width,height,depth,grow,outerU,outerV,64,64,false)
            };
        }
        return model;
    }
}
