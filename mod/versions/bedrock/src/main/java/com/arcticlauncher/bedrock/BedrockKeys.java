package com.arcticlauncher.bedrock;

import java.util.HashMap;
import java.util.Locale;
import java.util.Map;

/** Windows input to the shared core's GLFW codes and Minecraft binding names.
 * Native codes stay separate: VK_DELETE and the printable period, for example,
 * have different meanings despite sharing numeric values in the two APIs. */
final class BedrockKeys {
    private static final String KEYBOARD="key.keyboard.";
    private static final Map<Integer,Integer> CORE=new HashMap<Integer,Integer>();
    private static final Map<Integer,String> NAMES=new HashMap<Integer,String>();
    private static final Map<String,Integer> VIRTUAL=new HashMap<String,Integer>();

    static {
        for(int v='A';v<='Z';v++)bind(v,v,String.valueOf((char)(v+('a'-'A'))));
        for(int v='0';v<='9';v++)bind(v,v,String.valueOf((char)v));
        bind(32,32,"space");
        bind(0xDE,39,"apostrophe");bind(0xBC,44,"comma");bind(0xBD,45,"minus");
        bind(0xBE,46,"period");bind(0xBF,47,"slash");bind(0xBA,59,"semicolon");
        bind(0xBB,61,"equal");bind(0xDB,91,"left.bracket");bind(0xDC,92,"backslash");
        bind(0xDD,93,"right.bracket");bind(0xC0,96,"grave.accent");
        bind(27,256,"escape");bind(13,257,"enter");bind(9,258,"tab");bind(8,259,"backspace");
        bind(45,260,"insert");bind(46,261,"delete");bind(39,262,"right");bind(37,263,"left");
        bind(40,264,"down");bind(38,265,"up");bind(33,266,"page.up");bind(34,267,"page.down");
        bind(36,268,"home");bind(35,269,"end");
        bind(20,280,"caps.lock");bind(145,281,"scroll.lock");bind(144,282,"num.lock");
        bind(44,283,"print.screen");bind(19,284,"pause");
        for(int i=0;i<24;i++)bind(112+i,290+i,"f"+(i+1));
        for(int i=0;i<10;i++)bind(96+i,320+i,"keypad."+i);
        bind(110,330,"keypad.decimal");bind(111,331,"keypad.divide");
        bind(106,332,"keypad.multiply");bind(109,333,"keypad.subtract");bind(107,334,"keypad.add");
        bind(0xA0,340,"left.shift");bind(0xA2,341,"left.control");bind(0xA4,342,"left.alt");bind(91,343,"left.win");
        bind(0xA1,344,"right.shift");bind(0xA3,345,"right.control");bind(0xA5,346,"right.alt");bind(92,347,"right.win");
        bind(93,348,"menu");
        VIRTUAL.put("key.mouse.left",1);VIRTUAL.put("key.mouse.right",2);VIRTUAL.put("key.mouse.middle",4);
        VIRTUAL.put("key.mouse.4",5);VIRTUAL.put("key.mouse.5",6);
    }

    private BedrockKeys() {}

    private static void bind(int vk,int core,String suffix) {
        CORE.put(vk,core);NAMES.put(vk,KEYBOARD+suffix);VIRTUAL.put(KEYBOARD+suffix,vk);
    }

    static int core(int vk) {
        Integer value=CORE.get(vk);return value==null?-1:value;
    }

    static int virtual(String name) {
        Integer value=VIRTUAL.get(name);return value==null?-1:value;
    }

    static String name(int vk) {
        String value=NAMES.get(vk);return value==null?KEYBOARD+"unknown":value;
    }

    static String label(String name) {
        if(name==null || (KEYBOARD+"unknown").equals(name))return "Unbound";
        if(name.startsWith("key.mouse."))return "MOUSE "+name.substring("key.mouse.".length()).toUpperCase(Locale.ROOT);
        return name.replace(KEYBOARD,"").replace('.',' ').toUpperCase(Locale.ROOT);
    }
}
