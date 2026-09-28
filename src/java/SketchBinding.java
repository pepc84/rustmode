package processing.mode.rust;

/**
 * Represents a top-level {@code let [mut] name [: Type] = init;} binding
 * in the user's sketch, which is hoisted into the generated {@code SketchState}
 * struct.
 *
 * @param name    Variable name (Rust identifier).
 * @param mutable Whether {@code let mut} was used.
 * @param typeAnn Explicit type annotation text, or {@code null} if absent.
 * @param init    Initialiser expression text, or {@code null} if absent.
 */
public record SketchBinding(
  String  name,
  boolean mutable,
  String  typeAnn,
  String  init
) {}
