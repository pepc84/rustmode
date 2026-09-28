package processing.mode.rust;

import java.io.IOException;
import java.util.Collections;
import java.util.List;

/**
 * CargoException – thrown by {@link RustBuild} when {@code cargo build} exits
 * with a non-zero status.
 *
 * <p>Carries the list of structured diagnostics (already mapped back to sketch
 * tab/line coordinates) so that {@link RustEditor} can push them to the IDE
 * gutter as well as the status bar.
 */
public class CargoException extends IOException {

  private final List<RustProblem> diagnostics;

  public CargoException(String message, List<RustProblem> diagnostics) {
    super(message);
    this.diagnostics = Collections.unmodifiableList(diagnostics);
  }

  /** Structured cargo diagnostics, already mapped to sketch coordinates. */
  public List<RustProblem> diagnostics() {
    return diagnostics;
  }
}
