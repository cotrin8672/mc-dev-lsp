import java.nio.file.Files;
import java.nio.file.Path;

// Reproduce a language server exiting while its child retains the LSP pipes.
class TeardownProcess {
    public static void main(String[] args) throws Exception {
        if (args.length == 0) {
            Thread.sleep(60_000);
            return;
        }
        var java = Path.of(System.getProperty("java.home"), "bin", "java.exe").toString();
        var child = new ProcessBuilder(java, args[0]).inheritIO().start();
        Files.writeString(Path.of(args[1]), Long.toString(child.pid()));
        System.in.read();
    }
}
