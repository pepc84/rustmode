package ai.serenade.treesitter;

import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;

/**
 * Loads the native JNI bridge library.
 * Looks for it on java.library.path first; falls back to extracting from the jar.
 */
public class TreeSitter {
  private static volatile boolean loaded = false;

  public static synchronized void loadLib() {
    if (loaded) return;
    // Try java.library.path first (when running from the Processing IDE).
    try {
      System.loadLibrary("java-tree-sitter");
      loaded = true;
      return;
    } catch (UnsatisfiedLinkError ignored) {}
    // Fall back to extracting from the jar resource.
    String os   = System.getProperty("os.name", "").toLowerCase();
    String arch = System.getProperty("os.arch",  "").toLowerCase();
    String platform, name;
    if (os.contains("win")) {
      platform = "windows-x86-64"; name = "java-tree-sitter.dll";
    } else if (os.contains("mac")) {
      platform = arch.contains("aarch64") ? "macos-arm64" : "macos-x86-64";
      name     = "libjava-tree-sitter.dylib";
    } else {
      platform = "linux-x86-64"; name = "libjava-tree-sitter.so";
    }
    String res = "lib/" + platform + "/" + name;
    try (InputStream in = TreeSitter.class.getClassLoader().getResourceAsStream(res)) {
      if (in != null) {
        Path tmp = Path.of(System.getProperty("java.io.tmpdir")).resolve("jts-" + name);
        Files.copy(in, tmp, StandardCopyOption.REPLACE_EXISTING);
        System.load(tmp.toAbsolutePath().toString());
        loaded = true;
      }
    } catch (IOException | UnsatisfiedLinkError e) {
      // Will surface as UnsatisfiedLinkError when native methods are called.
    }
  }
}
