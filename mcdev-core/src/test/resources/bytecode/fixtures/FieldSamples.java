package io.github.mcdev.core.bytecode.fixtures;

public class FieldSamples {
    static int STATIC_FIELD = 1;
    int instanceField = 2;

    void accessFields() {
        int a = STATIC_FIELD;
        STATIC_FIELD = 3;
        int b = instanceField;
        instanceField = 4;
    }

    void duplicateFieldGets() {
        int a = STATIC_FIELD;
        int b = STATIC_FIELD;
    }
}
