package processing.mode.rust;

import processing.app.Base;
import processing.app.Sketch;
import processing.app.SketchCode;

import java.io.BufferedReader;
import java.io.File;
import java.io.FileWriter;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.List;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/**
 * RustBuild – drives the cargo build pipeline for a RustMode sketch.
 *
 * <p>Pipeline (mirrors CppBuild):
 * <ol>
 *   <li>Collect sketch tabs → concatenated source string.</li>
 *   <li>Parse with Tree-sitter via {@link RustAnalyzer}.</li>
 *   <li>Transform to a valid Rust binary via {@link RustEmitter}.</li>
 *   <li>Initialise / update the persistent cargo project in the build cache.</li>
 *   <li>Write the emitted source to {@code src/main.rs}.</li>
 *   <li>Run {@code cargo build [--release]}.</li>
 *   <li>Rewrite any rustc error messages back to sketch line numbers and
 *       strip fully-qualified crate paths before surfacing them in the IDE.</li>
 * </ol>
 *
 * <p>The cargo project is kept in
 * {@code ~/.processing/rust-build-cache/<sketch-name>/} so that
 * {@code target/} is preserved between runs, giving warm incremental builds.
 */
public class RustBuild {

  private static final java.util.logging.Logger LOG =
    java.util.logging.Logger.getLogger(RustBuild.class.getName());

  // ── Constants ───────────────────────────────────────────────────────────────

  /** Version stamp written to {@code .rustmode-version} in the cache project.
   *  Bump this to force a full project re-scaffold when the template changes. */
  private static final String RUSTMODE_VERSION = "0.1.0";

  /** Matches rustc error lines, e.g.
   *  {@code error[E0308]: --> src/main.rs:42:5} or the short form
   *  {@code src/main.rs:42:5: error: …}
   */
  private static final Pattern ERR_LINE_PATTERN =
    Pattern.compile("-->\\s+src/main\\.rs:(\\d+):(\\d+)");

  /** Matches the processing:: prefix that sketches should never see. */
  private static final Pattern CRATE_PREFIX_PATTERN =
    Pattern.compile("\\bprocessing::"); // e.g. processing::App

  // ── Fields ──────────────────────────────────────────────────────────────────

  private final Sketch sketch;
  private final RustMode mode;
  private final boolean release;

  /** Number of lines injected before the user's code in main.rs.
   *  Used to map rustc line numbers back to sketch lines. */
  private int headerLineCount = 0;

  /** Per-tab start lines in the concatenated source (1-based). */
  private int[] tabStartLines;

  // ── Construction ────────────────────────────────────────────────────────────

  public RustBuild(Sketch sketch, RustMode mode, boolean release) {
    this.sketch  = sketch;
    this.mode    = mode;
    this.release = release;
  }

  // ── Public entry-point ──────────────────────────────────────────────────────

  /**
   * Run the full build pipeline.
   *
   * @return path to the compiled binary, or {@code null} on failure.
   */
  public File build() throws IOException, InterruptedException, java.io.UnsupportedEncodingException {
    // 1 ── Concatenate sketch tabs ─────────────────────────────────────────────
    String sketchSource = collectTabs();

    // 2 ── Parse + analyse ────────────────────────────────────────────────────
    RustAnalyzer analyzer = new RustAnalyzer(sketchSource, sketch.getName());
    RustAnalysis  analysis = analyzer.analyse();

    // 3 ── Emit main.rs ───────────────────────────────────────────────────────
    RustEmitter emitter = new RustEmitter(analysis);
    String      mainRs  = emitter.emit();
    headerLineCount = emitter.getHeaderLineCount();

    // 4 ── Ensure persistent cargo project exists ──────────────────────────────
    File cacheProject = ensureCacheProject();

    // 5 ── Write src/main.rs ──────────────────────────────────────────────────
    File mainRsFile = new File(cacheProject, "src/main.rs");
    writeFile(mainRsFile, mainRs);

    // 6 ── cargo build ────────────────────────────────────────────────────────
    return runCargo(cacheProject);
  }

  // ── Step 1 – Collect tabs ───────────────────────────────────────────────────

  /**
   * Concatenate all sketch tabs into a single source string, tracking the
   * start line of each tab so we can map errors back to the right file.
   */
  private String collectTabs() {
    SketchCode[] codes = sketch.getCode();
    tabStartLines = new int[codes.length];
    StringBuilder sb = new StringBuilder();
    int lineCount = 1;

    for (int i = 0; i < codes.length; i++) {
      tabStartLines[i] = lineCount;
      String src = codes[i].getProgram();
      sb.append(src);
      if (!src.endsWith("\n")) sb.append('\n');
      // Count lines in this tab
      for (int c = 0; c < src.length(); c++) {
        if (src.charAt(c) == '\n') lineCount++;
      }
    }
    return sb.toString();
  }

  // ── Step 4 – Cache project ──────────────────────────────────────────────────

  /**
   * Return (creating if necessary) the persistent cargo project directory.
   *
   * <p>Layout:
   * <pre>
   * ~/.processing/rust-build-cache/&lt;sketch-name&gt;/
   *   .rustmode-version        ← recreated if stale
   *   Cargo.toml
   *   Cargo.lock               ← preserved (avoids re-resolution)
   *   processing-api/          ← copied from RustMode on creation / update
   *   src/
   *     main.rs                ← overwritten every build
   *   target/                  ← preserved (incremental builds)
   * </pre>
   */
  private File ensureCacheProject() throws IOException {
    File cacheRoot = new File(Base.getSketchbookFolder(),
                              "rust-build-cache/" + sanitiseName(sketch.getName()));
    cacheRoot.mkdirs();

    File versionStamp = new File(cacheRoot, ".rustmode-version");
    boolean needsScaffold = !versionStamp.exists()
      || !readFile(versionStamp).trim().equals(RUSTMODE_VERSION);

    // Also re-copy the processing crate if rebuild-engine.sh updated it since
    // we last scaffolded.  The script touches rust-build-cache/.lib-stamp.
    if (!needsScaffold) {
      File libStamp = new File(cacheRoot.getParentFile(), ".lib-stamp");
      if (libStamp.exists() && libStamp.lastModified() > versionStamp.lastModified()) {
        needsScaffold = true;
      }
    }

    if (needsScaffold) {
      scaffoldProject(cacheRoot);
      writeFile(versionStamp, RUSTMODE_VERSION);
    }

    // Always ensure src/ exists (target/ is cargo's responsibility).
    new File(cacheRoot, "src").mkdirs();

    return cacheRoot;
  }

  /** Create or replace the non-source scaffolding. */
  private void scaffoldProject(File root) throws IOException {
    // ── sketch Cargo.toml ─────────────────────────────────────────────────────
    // The processing crate lives at RustMode/src/lib.rs; we copy it into a
    // processing/ subdir of the cache project so the path dep is relative.
    String sketchBinName = sanitiseName(sketch.getName());
    String cargoToml =
      "[package]\n" +
      "name = \"" + sketchBinName + "\"\n" +
      "version = \"0.1.0\"\n" +
      "edition = \"2021\"\n" +
      "\n" +
      "[dependencies]\n" +
      "processing = { path = \"processing\" }\n" +
      "\n" +
      "[features]\n" +
      "default = []\n" +
      "# Build for browser: cargo build --target wasm32-unknown-unknown\\\n" +
      "#   --features wasm --no-default-features\n" +
      "wasm = [\"processing/wasm\"]\n" +
      "\n" +
      "[profile.dev]\n" +
      "opt-level = 1\n" +
      "\n" +
      "[profile.release]\n" +
      "opt-level = 3\n" +
      "lto = true\n" +
      "codegen-units = 1\n";

    writeFile(new File(root, "Cargo.toml"), cargoToml);

    // ── processing/ ───────────────────────────────────────────────────────────
    // The crate source lives in RustMode/src/lib.rs and RustMode/Cargo.toml.
    // All backend stubs are inlined as `mod` blocks in lib.rs — no crates/ dir.
    File modeRoot = mode.getFolder();
    File destCrate = new File(root, "processing");
    destCrate.mkdirs();
    new File(destCrate, "src").mkdirs();

    Files.copy(new File(modeRoot, "Cargo.toml").toPath(),
               new File(destCrate, "Cargo.toml").toPath(),
               StandardCopyOption.REPLACE_EXISTING);
    Files.copy(new File(modeRoot, "src/lib.rs").toPath(),
               new File(destCrate, "src/lib.rs").toPath(),
               StandardCopyOption.REPLACE_EXISTING);
  }

  // ── Step 6 – cargo build ────────────────────────────────────────────────────

  private File runCargo(File projectDir) throws IOException, InterruptedException {
    // ── Pre-warm: extract bundled dep rlibs on first run ─────────────────────────
    extractPrebuiltDeps();

    // ── Pre-flight: ensure cargo is on PATH ───────────────────────────────────
    if (!isCargoAvailable()) {
      throw new IOException(
        "cargo not found — install Rust from https://rustup.rs and restart Processing.");
    }

    List<String> cmd = new ArrayList<>();
    cmd.add("cargo");
    cmd.add("build");
    if (release) cmd.add("--release");
    cmd.add("--message-format=short");

    ProcessBuilder pb = new ProcessBuilder(cmd);
    pb.directory(projectDir);
    pb.redirectErrorStream(true);  // merge stderr into stdout

    // Propagate CARGO_HOME / RUSTUP_HOME from the environment; add sccache if
    // available so that the processing crate and minifb are cached across sketches.
    pb.environment().put("RUSTFLAGS",
      System.getenv().getOrDefault("RUSTFLAGS", "") + " -C incremental=true");

    // Point all sketches at a shared target dir so deps are compiled once.
    String sharedTarget = System.getProperty("user.home")
      + "/sketchbook/rust-build-cache/.shared-target";
    pb.environment().put("CARGO_TARGET_DIR", sharedTarget);

    Process proc = pb.start();

    // Stream output, rewriting error lines in real time.
    BufferedReader reader = new BufferedReader(
      new InputStreamReader(proc.getInputStream(), StandardCharsets.UTF_8));

    // Collect structured diagnostics for gutter markers; first error for status bar.
    List<RustProblem> diagnostics = new ArrayList<>();
    String firstError = null;
    String line;
    while ((line = reader.readLine()) != null) {
      String rewritten = rewriteCargoError(line);
      // Forward all output to the IDE console.
      mode.statusMessage(rewritten);

      if (rewritten.startsWith("error") || rewritten.startsWith("warning")) {
        // Try to extract tab + line from the rewritten error so we can build a marker.
        // After rewriting, error lines look like:
        //   error[E0308]: mismatched types
        //   --> Sketch.pde:12:5
        // We parse the --> line to get the gutter position.
        RustProblem dp = parseRewrittenDiagnostic(rewritten);
        if (dp != null) {
          diagnostics.add(dp);
        }
        if (firstError == null && rewritten.startsWith("error")) {
          firstError = rewritten;
        }
      }
    }

    int exitCode = proc.waitFor();
    if (exitCode != 0) {
      String msg = firstError != null
        ? firstError
        : "cargo build failed (exit " + exitCode + ") — see console for details";
      throw new CargoException(msg, diagnostics);
    }

    // Find the binary in the shared target dir.
    String profile   = release ? "release" : "debug";
    String sketchBin = sanitiseName(sketch.getName());
    String binaryName = isWindows() ? sketchBin + ".exe" : sketchBin;
    String sharedTargetForBin = System.getProperty("user.home")
      + "/sketchbook/rust-build-cache/.shared-target";
    File binary = new File(sharedTargetForBin,
      profile + "/" + binaryName);

    if (!binary.exists()) {
      throw new IOException("Build succeeded but binary not found: " + binary);
    }
    return binary;
  }

  // ── Diagnostic parsing ──────────────────────────────────────────────────────

  /** Matches rewritten cargo location lines: {@code --> Sketch.pde:12:5} */
  private static final Pattern REWRITTEN_LOCATION =
    Pattern.compile("-->\\s+(\\S+\\.(?:pde|prs|rs)):(\\d+):(\\d+)");

  /**
   * Try to build a {@link RustProblem} from a rewritten cargo output line.
   * Returns {@code null} if the line carries no parseable location.
   *
   * <p>After {@link #rewriteCargoError} runs, location lines look like:
   * <pre>  --> Sketch.pde:12:5</pre>
   * We match those and find the tab index by file name.
   */
  private RustProblem parseRewrittenDiagnostic(String line) {
    Matcher m = REWRITTEN_LOCATION.matcher(line);
    if (!m.find()) return null;

    String fileName = m.group(1);
    int    lineNum  = Integer.parseInt(m.group(2));  // 1-based
    int    col      = Integer.parseInt(m.group(3));  // 1-based

    // Find which tab this file name corresponds to.
    processing.app.SketchCode[] codes = sketch.getCode();
    int tabIndex = 0;
    for (int i = 0; i < codes.length; i++) {
      if (codes[i].getFileName().equals(fileName)) {
        tabIndex = i;
        break;
      }
    }

    boolean isErr = line.startsWith("error") ||
                    (line.contains("error") && !line.startsWith("warning"));
    return new RustProblem(tabIndex, lineNum - 1, col - 1,
                           line.trim(),
                           isErr ? RustProblem.ERROR : RustProblem.WARNING,
                           -1, -1);
  }

  // ── Error rewriting ─────────────────────────────────────────────────────────

  /**
   * Rewrite a rustc error line so that:
   * <ul>
   *   <li>Line numbers refer to the sketch tab, not main.rs.</li>
   *   <li>{@code processing::} crate prefixes are stripped.</li>
   * </ul>
   */
  private String rewriteCargoError(String line) {
    // Strip processing:: prefixes (e.g. "processing::App" → "App").
    line = CRATE_PREFIX_PATTERN.matcher(line).replaceAll("");

    Matcher m = ERR_LINE_PATTERN.matcher(line);
    if (m.find()) {
      int mainRsLine = Integer.parseInt(m.group(1));
      int sketchLine = mainRsLine - headerLineCount;
      if (sketchLine < 1) sketchLine = 1;

      // Determine which tab owns this line.
      int tabIndex = 0;
      for (int i = tabStartLines.length - 1; i >= 0; i--) {
        if (sketchLine >= tabStartLines[i]) {
          tabIndex = i;
          break;
        }
      }
      int lineInTab = sketchLine - tabStartLines[tabIndex] + 1;
      String tabName = sketch.getCode(tabIndex).getFileName();

      line = line.substring(0, m.start()) +
        "--> " + tabName + ":" + lineInTab + ":" + m.group(2) +
        line.substring(m.end());
    }
    return line;
  }

  // ── Utilities ───────────────────────────────────────────────────────────────

  /**
   * Extract the platform-specific prebuilt dep rlibs (bundled in the mode dir)
   * into the shared Cargo target so the first sketch build skips dep compilation.
   * Runs once per mode version; a stamp file prevents repeat extraction.
   */
  private void extractPrebuiltDeps() {
    try {
      String os   = System.getProperty("os.name",  "").toLowerCase();
      String arch = System.getProperty("os.arch",  "").toLowerCase();
      String platform;
      if (os.contains("win"))       platform = "windows-x86-64";
      else if (os.contains("mac"))  platform = arch.contains("aarch64") ? "macos-arm64" : "macos-x86-64";
      else                          platform = arch.contains("aarch64") ? "linux-arm64"  : "linux-x86-64";

      File modeDir = mode.getFolder();
      File bundle  = new File(modeDir, "prebuilt-deps/" + platform + ".tar.gz");
      if (!bundle.exists()) return;  // not bundled for this platform

      File sharedDeps = new File(System.getProperty("user.home")
        + "/sketchbook/rust-build-cache/.shared-target/debug/deps");

      File stamp = new File(System.getProperty("user.home")
        + "/sketchbook/rust-build-cache/.prebuilt-stamp-" + platform);

      // Re-extract if bundle is newer than stamp
      if (stamp.exists() && stamp.lastModified() >= bundle.lastModified()) return;

      sharedDeps.mkdirs();
      LOG.info("RustMode: extracting prebuilt deps for " + platform + "...");

      ProcessBuilder pb = new ProcessBuilder(
        "tar", "xzf", bundle.getAbsolutePath(),
        "--strip-components=1",
        "-C", sharedDeps.getAbsolutePath());
      pb.redirectErrorStream(true);
      Process p = pb.start();
      p.waitFor();

      stamp.getParentFile().mkdirs();
      stamp.createNewFile();
      LOG.info("RustMode: prebuilt deps extracted to " + sharedDeps);
    } catch (Exception e) {
      LOG.log(java.util.logging.Level.WARNING, "RustMode: could not extract prebuilt deps", e);
    }
  }

  /** Returns true if {@code cargo} is reachable on the current PATH. */
  private static boolean isCargoAvailable() {
    try {
      Process p = new ProcessBuilder("cargo", "--version")
        .redirectErrorStream(true)
        .start();
      p.getInputStream().transferTo(java.io.OutputStream.nullOutputStream());
      return p.waitFor() == 0;
    } catch (IOException | InterruptedException e) {
      return false;
    }
  }

  private static String sanitiseName(String name) {
    return name.replaceAll("[^A-Za-z0-9_\\-]", "_");
  }

  private static boolean isWindows() {
    return System.getProperty("os.name", "").toLowerCase().startsWith("win");
  }

  private static void writeFile(File f, String content) throws IOException {
    f.getParentFile().mkdirs();
    try (PrintWriter pw = new PrintWriter(new FileWriter(f, StandardCharsets.UTF_8))) {
      pw.print(content);
    }
  }

  private static String readFile(File f) throws IOException {
    return new String(Files.readAllBytes(f.toPath()), StandardCharsets.UTF_8);
  }

  /**
   * Recursively copy {@code src} directory to {@code dest}, preserving
   * structure.  Existing files are overwritten.
   */
  private static void copyDirectory(File src, File dest) throws IOException {
    if (!src.exists()) return;
    dest.mkdirs();
    File[] children = src.listFiles();
    if (children == null) return;
    for (File child : children) {
      File target = new File(dest, child.getName());
      if (child.isDirectory()) {
        copyDirectory(child, target);
      } else {
        Files.copy(child.toPath(), target.toPath(),
          StandardCopyOption.REPLACE_EXISTING);
      }
    }
  }
}
