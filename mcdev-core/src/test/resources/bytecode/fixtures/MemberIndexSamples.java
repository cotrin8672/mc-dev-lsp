package io.github.mcdev.core.bytecode.fixtures;

public class MemberIndexSamples extends BaseClass implements SampleInterface {
    public static final String STATIC_FIELD = "static";
    private int instanceField;

    public MemberIndexSamples() {
        instanceField = 0;
    }

    public void instanceMethod() {}

    public static void staticMethod() {}

    @Override
    public void interfaceMethod() {}

    private void hiddenMethod() {}
}

abstract class BaseClass {
    protected void baseMethod() {}
}

interface SampleInterface {
    void interfaceMethod();
}
