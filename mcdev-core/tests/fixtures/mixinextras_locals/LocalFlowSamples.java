/** Disposable oracle input covering frame joins, exception edges and wide locals. */
public class LocalFlowSamples {
    private final String value;
    public LocalFlowSamples(String value) { this.value = value; }
    public Object differentReferenceJoin(boolean flag, String first, Integer second) {
        Object selected;
        if (flag) selected = first; else selected = second;
        return selected;
    }
    public String sameReferenceJoin(boolean flag, String first, String second) {
        String selected;
        if (flag) selected = first; else selected = second;
        return selected;
    }
    public long loopAndWideSlots(long initial, double factor, int rounds) {
        long result = initial;
        for (int i = 0; i < rounds; i++) result += (long) (factor * i);
        return result;
    }
    public int exceptionEdges(String text) {
        int count = 1;
        try { count += text.length(); }
        catch (RuntimeException failure) { count += failure.hashCode(); }
        return count;
    }
    public Object arraysAndNull(boolean flag, String[] texts, int[][] numbers) {
        Object selected = flag ? texts : numbers;
        Object nullable = null;
        if (selected != null) nullable = selected;
        return nullable;
    }
    public static int switches(int selector) {
        int result;
        switch (selector) { case 0: result = 3; break; case 1: result = 7; break; default: result = 9; }
        return result;
    }
    public int unicodeLocalNames(int input) { int 数値 = input + value.length(); int Δ = 数値 * 2; return Δ; }
}
