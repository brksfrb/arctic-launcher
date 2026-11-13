package com.arcticlauncher.bedrock;

import com.arcticlauncher.client.looks.Attach;
import java.io.*;

/** Copied Java model visibility. Masks use Arctic Attach order, not an engine
 * or network bit layout: head, body, right arm, left arm, right leg, left leg. */
public final class JavaSkinVisibility {
    static final class Inputs {
        int schema;Integer baseParts;
        Boolean hat,jacket,rightSleeve,leftSleeve,rightPants,leftPants;
    }
    private final int base,outer;
    private JavaSkinVisibility(int base,int outer) {
        if(base<0||base>63||outer<0||outer>63)throw new IllegalArgumentException("invalid Java skin visibility mask");
        this.base=base;this.outer=outer;
    }
    public static JavaSkinVisibility fromMasks(int baseParts,int outerParts){return new JavaSkinVisibility(baseParts,outerParts);}
    public boolean baseShown(Attach part){return (base&(1<<part.ordinal()))!=0;}
    public boolean outerShown(Attach part){return (outer&(1<<part.ordinal()))!=0;}
    boolean same(JavaSkinVisibility other){return other!=null&&base==other.base&&outer==other.outer;}
    static JavaSkinVisibility parse(Inputs input) {
        if(input==null)return null;
        if(input.schema!=1||input.baseParts==null||input.hat==null||input.jacket==null||input.rightSleeve==null
                ||input.leftSleeve==null||input.rightPants==null||input.leftPants==null)
            throw new IllegalArgumentException("skin visibility requires base parts and all six outer flags");
        int outer=(input.hat?1:0)|(input.jacket?2:0)|(input.rightSleeve?4:0)
            |(input.leftSleeve?8:0)|(input.rightPants?16:0)|(input.leftPants?32:0);
        return fromMasks(input.baseParts,outer);
    }
    static void write(DataOutput output,JavaSkinVisibility value) throws IOException {
        output.writeByte(value==null?0:1);
        if(value!=null){output.writeByte(value.base);output.writeByte(value.outer);}
    }
    static JavaSkinVisibility read(DataInput input) throws IOException {
        int present=input.readUnsignedByte();if(present==0)return null;
        if(present!=1)throw new IOException("invalid recorded skin visibility presence");
        try{return fromMasks(input.readUnsignedByte(),input.readUnsignedByte());}
        catch(IllegalArgumentException invalid){throw new IOException("invalid recorded skin visibility",invalid);}
    }
}
