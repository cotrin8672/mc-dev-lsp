package org.eclipse.core.runtime;

public interface IProgressMonitor {
    void beginTask(String name, int totalWork);

    void done();

    void internalWorked(double work);

    void setTaskName(String name);

    void subTask(String name);

    void worked(int work);

    boolean isCanceled();
}
