package org.eclipse.jdt.core;

public interface ITypeHierarchy {
    IType[] getAllSubtypes(IType type);
}
