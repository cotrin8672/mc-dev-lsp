package org.eclipse.core.runtime;

public class NullProgressMonitor implements IProgressMonitor {
    @Override
    public void beginTask(String name, int totalWork) {
    }

    @Override
    public void done() {
    }

    @Override
    public void internalWorked(double work) {
    }

    @Override
    public void setTaskName(String name) {
    }

    @Override
    public void subTask(String name) {
    }

    @Override
    public void worked(int work) {
    }

    @Override
    public boolean isCanceled() {
        return false;
    }
}
