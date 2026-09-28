package processing.mode.rust;

import processing.app.Problem;

/**
 * RustProblem – adapts a {@link RustLinter.LintProblem} or a cargo error
 * to Processing's {@link Problem} interface so that the IDE can display it
 * as a gutter marker, underline, and error-table entry.
 *
 * <p>Processing 4 uses {@link Problem} in the editor's gutter via
 * {@code Editor.setProblem(Problem)} and {@code Editor.clearProblems()}.
 * The line number passed to the interface is 0-based; {@link RustLinter}
 * produces 1-based line numbers, so we subtract one here.
 */
public class RustProblem implements Problem {

  // Severity constants — mirror Problem.ERROR / Problem.WARNING values (1 / 2).
  public static final int ERROR   = 1;
  public static final int WARNING = 2;

  private final int     tabIndex;
  private final int     lineZeroBased;  // 0-based, as Problem requires
  private final int     column;         // 0-based; 0 if unknown
  private final String  message;
  private final int     severity;       // ERROR or WARNING
  private final int     startOffset;    // character offset in the tab source; -1 if unknown
  private final int     stopOffset;     // character offset in the tab source; -1 if unknown

  // ── Factory: from a LintProblem ───────────────────────────────────────────────

  /**
   * Build an ERROR-level {@link RustProblem} from a linter diagnostic.
   * {@code lp.line()} is 1-based; we convert to 0-based here.
   */
  public static RustProblem fromLint(RustLinter.LintProblem lp) {
    return new RustProblem(lp.tabIndex(), lp.line() - 1, 0,
                           lp.message(), ERROR, -1, -1);
  }

  /**
   * Build a WARNING-level {@link RustProblem} from a linter diagnostic.
   */
  public static RustProblem warnFromLint(RustLinter.LintProblem lp) {
    return new RustProblem(lp.tabIndex(), lp.line() - 1, 0,
                           lp.message(), WARNING, -1, -1);
  }

  // ── Constructor ───────────────────────────────────────────────────────────────

  public RustProblem(int tabIndex, int lineZeroBased, int column,
                     String message, int severity,
                     int startOffset, int stopOffset) {
    this.tabIndex      = tabIndex;
    this.lineZeroBased = Math.max(0, lineZeroBased);
    this.column        = Math.max(0, column);
    this.message       = message;
    this.severity      = severity;
    this.startOffset   = startOffset;
    this.stopOffset    = stopOffset;
  }

  // ── Problem interface ─────────────────────────────────────────────────────────

  @Override public int    getTabIndex()    { return tabIndex; }
  public int getLine()        { return lineZeroBased; }
  @Override public int getLineNumber()   { return lineZeroBased; }
  public int getColumn()      { return column; }
  @Override public String getMessage()     { return message; }
  public int getSeverity()    { return severity; }
  @Override public boolean isError()       { return severity == ERROR; }
  @Override public boolean isWarning()     { return severity == WARNING; }

  /**
   * Character offset of the start of the problem within the tab's source.
   * Returns -1 when not known; the IDE tolerates this (it falls back to
   * line-level highlighting).
   */
  @Override public int getStartOffset() { return startOffset; }
  @Override public int getStopOffset()  { return stopOffset;  }

  // ── Helpers ───────────────────────────────────────────────────────────────────

  @Override
  public String toString() {
    return (isError() ? "error" : "warning") + " [tab=" + tabIndex +
           " line=" + (lineZeroBased + 1) + "]: " + message;
  }
}
