package processing.mode.rust;

import processing.app.Base;
import processing.app.Preferences;
import processing.app.ui.Editor;
import processing.app.ui.EditorException;
import processing.app.ui.EditorState;

import javax.swing.*;
import java.io.File;
import java.util.List;
import java.util.logging.Level;
import java.util.logging.Logger;
import processing.app.Formatter;
import processing.app.ui.EditorToolbar;
import javax.swing.JMenu;

/**
 * RustEditor – the sketch editor for RustMode.
 *
 * <p>Extends Processing's {@link Editor} base class and wires the Run / Stop
 * toolbar buttons to {@link RustBuild}.
 */
public class RustEditor extends Editor {

  private static final Logger LOG = Logger.getLogger(RustEditor.class.getName());

  private RustBuild        currentBuild;
  private Process          runningProcess;
  private RustInputHandler inputHandler;

  protected RustEditor(Base base, String path, EditorState state, RustMode mode)
      throws EditorException {
    super(base, path, state, mode);
    inputHandler = new RustInputHandler(this);
  }

  @Override
  public void deactivateRun() {}

  // ── Mode accessors ────────────────────────────────────────────────────────────

  public RustMode getRustMode() {
    return (RustMode) getMode();
  }

  // ── Toolbar actions ───────────────────────────────────────────────────────────

    public void handleRun(boolean present, Runnable runAction, Runnable stopAction) {
    // Sync the active tab's editor Document to SketchCode so getProgram()
    // returns current content. The base handleRun() does this; we bypass it.
    sketch.getCurrentCode().setProgram(getText());

    // Kick the build off on a background thread so the EDT stays responsive.
    new Thread(() -> {
      try {
        statusNotice("Building with cargo…");

        // ── Pre-build lint (Tree-sitter, fast) ──────────────────────────────
        String source = String.join("\n",
          java.util.Arrays.stream(sketch.getCode())
            .map(processing.app.SketchCode::getProgram)
            .toArray(String[]::new));
        RustAnalyzer analyzer = new RustAnalyzer(source, sketch.getName());
        RustAnalysis  analysis = analyzer.analyse();
        RustLinter    linter   = new RustLinter(sketch, analysis);
        List<RustLinter.LintProblem> lintProblems = linter.lint();
        if (!lintProblems.isEmpty()) {
          // Log and surface the first problem in the status bar.
          for (RustLinter.LintProblem p : lintProblems) {
            String tabName = sketch.getCode(p.tabIndex()).getFileName();
            LOG.warning("Lint: " + tabName + ":" + p.line() + ": " + p.message());
          }
          RustLinter.LintProblem first = lintProblems.get(0);
          String tabName = sketch.getCode(first.tabIndex()).getFileName();
          statusError(tabName + ":" + first.line() + ": " + first.message());
          stopAction.run();
          return;
        }

        boolean release = Preferences.getBoolean("build.release");
        currentBuild = new RustBuild(sketch, getRustMode(), release);
        File binary = currentBuild.build();

        statusNotice("Running " + binary.getName() + "…");
        runBinary(binary, stopAction);

      } catch (CargoException e) {
        LOG.log(Level.WARNING, "cargo build failed", e);
        statusError(e.getMessage() != null ? e.getMessage() : "cargo build failed");
        stopAction.run();
      } catch (java.io.UnsupportedEncodingException e) {
        statusError("Tree-sitter encoding error: " + e.getMessage());
        stopAction.run();
      } catch (Exception e) {
        LOG.log(Level.WARNING, "Build failed", e);
        statusError(e.getMessage() != null ? e.getMessage() : e.toString());
        stopAction.run();
      }
    }, "RustMode-build").start();
  }

    public void handleStop() {
    if (runningProcess != null && runningProcess.isAlive()) {
      runningProcess.destroy();
      runningProcess = null;
    }
    statusNotice("Stopped.");
  }

  // ── Binary launcher ───────────────────────────────────────────────────────────

  private void runBinary(File binary, Runnable stopAction) throws Exception {
    ProcessBuilder pb = new ProcessBuilder(binary.getAbsolutePath());
    pb.directory(sketch.getFolder());
    pb.redirectErrorStream(true);

    runningProcess = pb.start();

    // Stream sketch stdout to the IDE console.
    // System.out is redirected to the Processing console panel at runtime.
    new Thread(() -> {
      try (var reader = new java.io.BufferedReader(
          new java.io.InputStreamReader(runningProcess.getInputStream()))) {
        String line;
        while ((line = reader.readLine()) != null) {
          System.out.println(line);
        }
      } catch (Exception ignored) {}
      // Notify the toolbar that the run is finished.
      SwingUtilities.invokeLater(stopAction);
    }, "RustMode-runner").start();
  }

  // ── Syntax keywords ───────────────────────────────────────────────────────────

  @Override
  public String getCommentPrefix() {
    return "//";
  }

  // ── Console helpers (delegating to base) ──────────────────────────────────────

  public void statusMessage(String msg) {
    // Route cargo output to the IDE console.
    // error/warning lines go to stderr (shown in red in the Processing console).
    if (msg.startsWith("error") || msg.startsWith("warning")) {
      System.err.println(msg);
    } else if (!msg.isBlank()) {
      System.out.println(msg);
    }
  }

  // ── Required abstract stubs ───────────────────────────────────────────────────

  @Override
  public void internalCloseRunner() {
    if (runningProcess != null && runningProcess.isAlive()) {
      runningProcess.destroy();
      runningProcess = null;
    }
  }


  // ── Abstract method stubs required by Editor ─────────────────────────────────

  @Override
  public EditorToolbar createToolbar() {
    return new processing.app.ui.EditorToolbar(this) {
      @Override public void handleRun(int modifiers)  { RustEditor.this.handleRun(false, () -> {}, () -> {}); }
      @Override public void handleStop()               { RustEditor.this.handleStop(); }
    };
  }

  @Override
  public Formatter createFormatter() {
    return null; // No auto-format for Rust (rustfmt can be added later)
  }

  @Override
  public JMenu buildFileMenu()    { return buildFileMenu(new javax.swing.JMenuItem[0]); }

  @Override
  public JMenu buildSketchMenu() { return buildSketchMenu(new javax.swing.JMenuItem[0]); }

  @Override
  public void handleImportLibrary(String jarPath) {
    // Rust sketches don't use Processing libraries; no-op.
  }

  @Override
  public JMenu buildHelpMenu()   { return new JMenu("Help"); }

}
