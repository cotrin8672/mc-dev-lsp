package org.eclipse.jdt.core;

import org.eclipse.core.runtime.IProgressMonitor;

public interface IType {
    ITypeHierarchy newTypeHierarchy(IJavaProject project, IProgressMonitor monitor);

    String getFullyQualifiedName();

    String getFullyQualifiedName(char dollarSeparator);

    IAnnotation[] getAnnotations();
}
