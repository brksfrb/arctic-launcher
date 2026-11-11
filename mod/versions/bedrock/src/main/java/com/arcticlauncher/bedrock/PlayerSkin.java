package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.Attach;
import com.arcticlauncher.client.looks.CuboidModel;
import com.arcticlauncher.client.looks.Geometry;
import com.google.gson.JsonParser;
import java.util.EnumMap;
import java.util.Map;

/** Legacy cosmetic-box skin builder retained for preview/reference callers.
 * Normal avatar emission uses JavaPlayerMesh's ModelPart face/UV layout.
 * These boxes are in the same part coordinates as SkinLimbs.
 * Only a bending emote uses split boxes: ordinary skin draws have no elbow,
 * knee or waist caps inside the player. UVs also apply to normalized HD skins. */
final class PlayerSkin {
    private static final Map<Attach,CuboidModel.Piece[][]> CACHE=new EnumMap<Attach,CuboidModel.Piece[][]>(Attach.class);

    private PlayerSkin() {}

    static synchronized CuboidModel.Piece piece(Attach part,boolean slim,boolean outer) {
        CuboidModel.Piece[][] variants=CACHE.get(part);
        if(variants==null) {
            variants=new CuboidModel.Piece[2][2];CACHE.put(part,variants);
        }
        int s=slim&&(part==Attach.RIGHT_ARM||part==Attach.LEFT_ARM)?1:0;
        int o=outer?1:0;
        if(variants[s][o]==null) variants[s][o]=CuboidModel.bake(Geometry.parse(new JsonParser().parse(json(part,s==1,outer))),null).get(0);
        return variants[s][o];
    }

    private static String json(Attach part,boolean slim,boolean outer) {
        String name;
        float x,y,z,inflate;
        int w,h,d,u,v,ou,ov;
        switch(part) {
            case HEAD:
                name="head";x=-4;y=24;z=-4;w=8;h=8;d=8;u=0;v=0;ou=32;ov=0;inflate=.5f;break;
            case BODY:
                name="body";x=-4;y=12;z=-2;w=8;h=12;d=4;u=16;v=16;ou=16;ov=32;inflate=.25f;break;
            case RIGHT_ARM:
                name="rightArm";w=slim?3:4;x=-4-w;y=12;z=-2;h=12;d=4;u=40;v=16;ou=40;ov=32;inflate=.25f;break;
            case LEFT_ARM:
                name="leftArm";w=slim?3:4;x=4;y=12;z=-2;h=12;d=4;u=32;v=48;ou=48;ov=48;inflate=.25f;break;
            case RIGHT_LEG:
                name="rightLeg";x=part.x-2;y=0;z=-2;w=4;h=12;d=4;u=0;v=16;ou=0;ov=32;inflate=.25f;break;
            case LEFT_LEG:
                name="leftLeg";x=part.x-2;y=0;z=-2;w=4;h=12;d=4;u=16;v=48;ou=0;ov=48;inflate=.25f;break;
            default: throw new IllegalArgumentException("unknown player part");
        }
        String cubes=cube(x,y,z,w,h,d,u,v,0);
        if(outer)cubes+=","+cube(x,y,z,w,h,d,ou,ov,inflate);
        return "{\"minecraft:geometry\":[{\"description\":{\"texture_width\":64,\"texture_height\":64},\"bones\":["
            +"{\"name\":\""+name+"\",\"pivot\":["+part.x+","+(24-part.y)+","+part.z+"],\"cubes\":["+cubes+"]}]}]}";
    }

    private static String cube(float x,float y,float z,int w,int h,int d,int u,int v,float inflate) {
        return "{\"origin\":["+x+","+y+","+z+"],\"size\":["+w+","+h+","+d+"],\"uv\":["+u+","+v+"],\"inflate\":"+inflate+"}";
    }
}
