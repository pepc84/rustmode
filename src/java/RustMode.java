package processing.mode.rust;

import processing.app.Base;
import processing.app.Mode;
import processing.app.ui.Editor;
import processing.app.ui.EditorException;
import processing.app.ui.EditorState;

import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.logging.Level;
import java.util.logging.Logger;

/**
 * RustMode, the Processing 4 mode plugin entry point.
 *
 * <p>Registered in {@code mode.properties}. Processing creates it once per IDE
 * session and asks it for editors, keywords and examples.
 */
public class RustMode extends Mode {

  private static final Logger LOG = Logger.getLogger(RustMode.class.getName());

  /** True once the tree-sitter native lib is loaded and parsing will work. */
  private static volatile boolean nativeLoaded = false;

  public RustMode(Base base, File folder) {
    super(base, folder);
    loadNativeTreeSitter();
  }

  public static boolean isNativeLoaded() {
    return nativeLoaded;
  }

  // ── Native library loading ────────────────────────────────────────────────────

  /**
   * Load {@code lib/<platform>/libjava-tree-sitter} from the jar.
   *
   * <p>That one library holds the serenade JNI bridge, the tree-sitter runtime
   * and the Rust grammar, all linked statically by {@code native/build.sh}.
   * Nothing is taken from the system, so a distro tree-sitter update can't
   * break the mode.
   */
  private void loadNativeTreeSitter() {
    String os   = System.getProperty("os.name", "").toLowerCase();
    String arch = System.getProperty("os.arch", "").toLowerCase();
    boolean arm = arch.contains("aarch64") || arch.contains("arm64");

    final String platform, libName;
    if (os.contains("win")) {
      platform = "windows-x86-64";
      libName  = "java-tree-sitter.dll";
    } else if (os.contains("mac")) {
      platform = arm ? "macos-arm64" : "macos-x86-64";
      libName  = "libjava-tree-sitter.dylib";
    } else {
      platform = arm ? "linux-arm64" : "linux-x86-64";
      libName  = "libjava-tree-sitter.so";
    }

    String resource = "lib/" + platform + "/" + libName;
    try (InputStream in = RustMode.class.getClassLoader().getResourceAsStream(resource)) {
      if (in == null) {
        LOG.warning("RustMode: " + resource + " not in jar. Run native/build.sh, then gradle jar.");
        return;
      }
      // Per-user file name so two users on one machine don't fight over /tmp.
      Path tmp = Path.of(System.getProperty("java.io.tmpdir"))
        .resolve("rustmode-" + System.getProperty("user.name", "user") + "-" + libName);
      Files.copy(in, tmp, StandardCopyOption.REPLACE_EXISTING);
      System.load(tmp.toAbsolutePath().toString());
      nativeLoaded = true;
      LOG.info("RustMode: loaded " + tmp);
    } catch (IOException e) {
      LOG.log(Level.WARNING, "RustMode: could not extract " + libName, e);
    } catch (Throwable t) {
      // UnsatisfiedLinkError is an Error; never let it take the mode down.
      LOG.log(Level.WARNING, "RustMode: could not load " + libName
        + ". Sketches can't be parsed until this is fixed.", t);
    }
  }

  // ── Mode identity ────────────────────────────────────────────────────────────

  @Override
  public String getTitle() {
    return "Rust";
  }

  @Override
  public String[] getIgnorable() {
    return new String[0];
  }

  @Override
  public String getDefaultExtension() {
    return "rs";
  }

  @Override
  public String[] getExtensions() {
    return new String[]{"rs", "pde"};
  }

  // ── Editor factory ────────────────────────────────────────────────────────────

  @Override
  public Editor createEditor(Base base, String path, EditorState state)
      throws EditorException {
    return new RustEditor(base, path, state, this);
  }

  // ── Example sketches ─────────────────────────────────────────────────────────

  @Override
  public File[] getExampleCategoryFolders() {
    return new File[]{new File(getFolder(), "examples")};
  }

  // ── Status helper (used by RustBuild) ─────────────────────────────────────────

  /** Send a line of cargo output to every open RustMode editor. */
  public void statusMessage(String msg) {
    for (Editor ed : base.getEditors()) {
      if (ed instanceof RustEditor re) {
        re.statusMessage(msg);
      }
    }
  }
}
