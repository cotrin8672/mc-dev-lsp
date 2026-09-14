package org.eclipse.jdt.core;

public interface IAnnotation {
    boolean exists();

    String getElementName();

    IMemberValuePair[] getMemberValuePairs();
}
