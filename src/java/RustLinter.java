package processing.mode.rust;

import processing.app.Sketch;
import processing.app.SketchCode;

import java.util.ArrayList;
import java.util.List;
import java.util.regex.Pattern;

/**
 * RustLinter – lightweight parse-time problem reporter for the RustMode editor.
 *
 * <p>Unlike CppMode's equivalent, which runs the C pre-processor for early
 * diagnostics, RustLinter uses the Tree-sitter parse tree to catch a small set
 * of problems before cargo is even invoked:
 *
 * <ul>
 *   <li>Missing {@code fn draw()} – every sketch needs one.</li>
 *   <li>Top-level {@code let} bindings with no type annotation <em>and</em> no
 *       initialiser (Rust cannot infer the type; the field declaration in
 *       {@code SketchState} will fail to compile).</li>
 *   <li>ERROR nodes produced by the Tree-sitter parser (syntax errors).</li>
 * </ul>
 *
 * <p>Both camelCase ({@code mousePressed}) and snake_case ({@code mouse_pressed})
 * lifecycle names are accepted and normalised by {@link RustAnalyzer}, so the
 * linter no longer warns about naming style for lifecycle callbacks.</p>
 *
 * <p>Problems are returned as {@link LintProblem} records carrying the sketch
 * tab index, 1-based line number, and a human-readable message.  {@link RustEditor}
 * passes these to the IDE's problem marker API to underline them in the gutter.
 */
public class RustLinter {

  // ── Fields ────────────────────────────────────────────────────────────────────

  private final Sketch      sketch;
  private final RustAnalysis analysis;

  // ── Construction ──────────────────────────────────────────────────────────────

  public RustLinter(Sketch sketch, RustAnalysis analysis) {
    this.sketch   = sketch;
    this.analysis = analysis;
  }

  // ── Public API ────────────────────────────────────────────────────────────────

  /** Run all checks and return the list of problems found (empty if clean). */
  public List<LintProblem> lint() {
    List<LintProblem> problems = new ArrayList<>();

    checkParseErrors(problems);
    // checkDrawPresent: disabled — cargo reports missing draw() more clearly.
    checkBindingTypes(problems);
    // checkCamelLifecycle: removed — both camelCase and snake_case lifecycle
    // names are accepted and normalised by RustAnalyzer.

    return problems;
  }

  // ── Checks ────────────────────────────────────────────────────────────────────

  /** Surface a generic syntax-error notice when Tree-sitter found ERROR/MISSING nodes. */
  private void checkParseErrors(List<LintProblem> problems) {
    if (analysis.hasParseErrors()) {
      problems.add(new LintProblem(0, 1,
        "Sketch contains a syntax error — fix it before running."));
    }
  }

  /** Every sketch must define {@code fn draw()}. */
  private void checkDrawPresent(List<LintProblem> problems) {
    if (analysis.hasDraw()) return;
    // Belt-and-suspenders: also scan raw source in case the analyzer missed it.
    java.util.regex.Pattern pat = java.util.regex.Pattern.compile("\\bfn\\s+draw\\s*\\(");
    for (SketchCode code : sketch.getCode()) {
      String prog = code.getProgram();
      if (prog != null && pat.matcher(prog).find()) return;
    }
    problems.add(new LintProblem(0, 1,
      "Sketch is missing fn draw() — add a draw function to run the sketch."));
  }

  /**
   * Top-level bindings with no type annotation and no initialiser cannot be
   * emitted as SketchState fields; warn the user to add a type annotation.
   */
  private void checkBindingTypes(List<LintProblem> problems) {
    for (SketchBinding b : analysis.bindings()) {
      if (b.typeAnn() == null && b.init() == null) {
        int[] loc = findBindingLine(b.name());
        problems.add(new LintProblem(loc[0], loc[1],
          "Sketch variable '" + b.name() + "' needs a type annotation or an initialiser: " +
          "let " + b.name() + ": YourType = …;"));
      }
    }
  }

  // ── Helper: find which tab + line a binding name appears on ────────────────────

  private int[] findBindingLine(String name) {
    SketchCode[] codes = sketch.getCode();
    Pattern pat = Pattern.compile("\\blet\\s+(?:mut\\s+)?" + Pattern.quote(name) + "\\b");
    for (int t = 0; t < codes.length; t++) {
      String[] lines = codes[t].getProgram().split("\n", -1);
      for (int ln = 0; ln < lines.length; ln++) {
        if (pat.matcher(lines[ln]).find()) {
          return new int[]{t, ln + 1};
        }
      }
    }
    return new int[]{0, 1}; // fallback
  }

  // ── LintProblem ────────────────────────────────────────────────────────────────

  /**
   * A single diagnostic produced by the linter.
   *
   * @param tabIndex  0-based index into {@code sketch.getCode()}.
   * @param line      1-based line number within that tab.
   * @param message   Human-readable description of the problem.
   */
  public record LintProblem(int tabIndex, int line, String message) {}
}
