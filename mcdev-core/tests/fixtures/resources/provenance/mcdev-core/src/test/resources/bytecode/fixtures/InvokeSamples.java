package io.github.mcdev.core.bytecode.fixtures;

public class InvokeSamples {
    private String field = "test";

    public void virtualInvoke() {
        field.length();
        System.out.println("hello");
    }

    public static void staticInvoke() {
        Math.abs(1);
        Integer.valueOf(2);
    }

    public void specialInvoke() {
        super.toString();
        new String("test");
    }

    public void interfaceInvoke() {
        Runnable r = () -> {};
        r.run();
    }

    public void duplicateInvokes() {
        Math.abs(1);
        Math.abs(2);
        Math.abs(3);
    }
}
