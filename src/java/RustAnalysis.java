package processing.mode.rust;

import java.util.List;
import java.util.Map;

/**
 * Plain value object produced by {@link RustAnalyzer} and consumed by
 * {@link RustEmitter}.
 */
public record RustAnalysis(
  /** All top-level sketch bindings, in declaration order. */
  List<SketchBinding> bindings,
  /** Lifecycle function source texts, keyed by name (e.g. "draw"). */
  Map<String, String> lifecycleFns,
  /** User-defined structs, helpers, {@code use} statements, etc. – emitted verbatim. */
  List<String> passthroughItems,
  /** Human-readable sketch name (used as the window title). */
  String sketchName,
  /** Locations (1-based {row, col}) of ERROR/MISSING nodes found during parsing. */
  List<int[]> parseErrorLocations
) {
  /** @return true if the parse found any ERROR or MISSING nodes. */
  public boolean hasParseErrors()     { return !parseErrorLocations.isEmpty(); }

  /** @return true if the sketch defines a {@code setup()} function. */
  public boolean hasSetup()        { return lifecycleFns.containsKey("setup"); }
  /** @return true if the sketch defines a {@code draw()} function. */
  public boolean hasDraw()         { return lifecycleFns.containsKey("draw"); }
  public boolean hasMousePressed() { return lifecycleFns.containsKey("mouse_pressed"); }
  public boolean hasMouseReleased(){ return lifecycleFns.containsKey("mouse_released"); }
  public boolean hasMouseMoved()   { return lifecycleFns.containsKey("mouse_moved"); }
  public boolean hasMouseDragged() { return lifecycleFns.containsKey("mouse_dragged"); }
  public boolean hasMouseWheel()   { return lifecycleFns.containsKey("mouse_wheel"); }
  public boolean hasKeyPressed()   { return lifecycleFns.containsKey("key_pressed"); }
  public boolean hasKeyReleased()  { return lifecycleFns.containsKey("key_released"); }
  public boolean hasKeyTyped()     { return lifecycleFns.containsKey("key_typed"); }
}
