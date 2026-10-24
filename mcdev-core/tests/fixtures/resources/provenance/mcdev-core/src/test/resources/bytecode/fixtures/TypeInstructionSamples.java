package io.github.mcdev.core.bytecode.fixtures;

import java.util.List;

public class TypeInstructionSamples {
    void typeInstructions(Object value, int length) {
        String s = (String) value;
        boolean isList = value instanceof List;
        new StringBuilder();
        String[] arr = new String[length];
    }
}
