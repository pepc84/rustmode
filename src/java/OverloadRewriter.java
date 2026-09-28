package processing.mode.rust;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Set;

/**
 * Lets sketches call engine functions the Processing way.
 *
 * <p>Rust has no overloading, so the engine exposes {@code fill}, {@code fill_rgb},
 * {@code fill_rgba} and so on. This pass picks the right one from the argument
 * count, so {@code fill(255, 120, 0)} becomes {@code fill_rgb(255.0, 120.0, 0.0)}.
 * It also turns bare integer literals into floats for engine calls, since Rust
 * won't pass {@code 255} to an {@code f32} parameter.
 *
 * <p>Works on text, not the parse tree, and never adds or removes newlines, so
 * error line numbers stay correct. Strings, chars and comments are skipped.
 */
final class OverloadRewriter {

  /** name -> (argument count -> engine function). */
  private static final Map<String, Map<Integer, String>> OVERLOADS = Map.of(
    "fill",       Map.of(2, "fill_ga",       3, "fill_rgb",       4, "fill_rgba"),
    "stroke",     Map.of(2, "stroke_ga",     3, "stroke_rgb",     4, "stroke_rgba"),
    "background", Map.of(2, "background_ga", 3, "background_rgb", 4, "background_rgba"),
    "rect",       Map.of(5, "rect_rounded"),
    "scale",      Map.of(2, "scale_xy"),
    "translate",  Map.of(3, "translate_3d"),
    "random",     Map.of(2, "random_range"),
    "noise",      Map.of(2, "noise2"),
    "begin_shape", Map.of(1, "begin_shape_kind"),
    "end_shape",  Map.of(1, "end_shape_mode")
  );

  /** Engine functions whose numeric parameters are all f32. */
  private static final Set<String> FLOAT_ARGS = Set.of(
    "fill", "fill_ga", "fill_rgb", "fill_rgba",
    "stroke", "stroke_ga", "stroke_rgb", "stroke_rgba",
    "background", "background_ga", "background_rgb", "background_rgba",
    "ellipse", "circle", "rect", "rect_rounded", "square", "line", "point",
    "triangle", "quad", "arc", "translate", "translate_3d", "rotate",
    "scale", "scale_xy", "stroke_weight", "text", "text_size", "frame_rate",
    "random", "random_range", "noise", "noise2",
    "vertex", "bezier_vertex", "quadratic_vertex", "curve_vertex"
  );

  private OverloadRewriter() {}

  static String rewrite(String src) {
    StringBuilder out = new StringBuilder(src.length() + 64);
    int i = 0, n = src.length();
    while (i < n) {
      char c = src.charAt(i);
      int skip = skipNonCode(src, i);
      if (skip > i) { out.append(src, i, skip); i = skip; continue; }
      if (Character.isJavaIdentifierStart(c) && (i == 0 || !isIdentOrPath(src.charAt(i - 1)))) {
        int j = i;
        while (j < n && Character.isJavaIdentifierPart(src.charAt(j))) j++;
        String name = src.substring(i, j);
        int k = j;
        while (k < n && (src.charAt(k) == ' ' || src.charAt(k) == '\t')) k++;
        boolean isCall = k < n && src.charAt(k) == '('
          && (OVERLOADS.containsKey(name) || FLOAT_ARGS.contains(name))
          && !precededByFn(src, i);
        if (isCall) {
          int close = matchParen(src, k);
          if (close > 0) {
            List<String> args = splitArgs(src.substring(k + 1, close));
            boolean empty = args.size() == 1 && args.get(0).isBlank();
            int count = empty ? 0 : args.size();
            String target = OVERLOADS.getOrDefault(name, Map.of()).getOrDefault(count, name);
            List<String> fixed = new ArrayList<>();
            for (String a : args) {
              String r = rewrite(a);
              if (FLOAT_ARGS.contains(target) && !"size".equals(target)) r = floatLiteral(r);
              fixed.add(r);
            }
            out.append(target).append(src, j, k + 1).append(String.join(",", fixed)).append(')');
            i = close + 1;
            continue;
          }
        }
        out.append(name);
        i = j;
        continue;
      }
      out.append(c);
      i++;
    }
    return out.toString();
  }

  /** {@code 255} -> {@code 255.0}, {@code -3} -> {@code -3.0}; keeps surrounding whitespace. */
  private static String floatLiteral(String arg) {
    String t = arg.strip();
    if (!t.matches("-?\\d+")) return arg;
    int lead = arg.indexOf(t);
    return arg.substring(0, lead) + t + ".0" + arg.substring(lead + t.length());
  }

  private static boolean isIdentOrPath(char c) {
    return Character.isJavaIdentifierPart(c) || c == '.' || c == ':';
  }

  private static boolean precededByFn(String s, int i) {
    int p = i - 1;
    while (p >= 0 && Character.isWhitespace(s.charAt(p))) p--;
    return p >= 1 && s.startsWith("fn", p - 1) && (p < 2 || !Character.isJavaIdentifierPart(s.charAt(p - 2)));
  }

  /** Index of the ')' matching the '(' at {@code open}, or -1. */
  private static int matchParen(String s, int open) {
    int depth = 0;
    for (int i = open; i < s.length(); ) {
      int skip = skipNonCode(s, i);
      if (skip > i) { i = skip; continue; }
      char c = s.charAt(i);
      if (c == '(' || c == '[' || c == '{') depth++;
      else if (c == ')' || c == ']' || c == '}') { if (--depth == 0) return i; }
      i++;
    }
    return -1;
  }

  /** Split on top-level commas, keeping each piece's original text. */
  private static List<String> splitArgs(String s) {
    List<String> parts = new ArrayList<>();
    int depth = 0, start = 0;
    for (int i = 0; i < s.length(); ) {
      int skip = skipNonCode(s, i);
      if (skip > i) { i = skip; continue; }
      char c = s.charAt(i);
      if (c == '(' || c == '[' || c == '{') depth++;
      else if (c == ')' || c == ']' || c == '}') depth--;
      else if (c == ',' && depth == 0) { parts.add(s.substring(start, i)); start = i + 1; }
      i++;
    }
    parts.add(s.substring(start));
    // A trailing comma leaves an empty last piece; drop it so fill(1, 2, 3,) still counts 3.
    if (parts.size() > 1 && parts.get(parts.size() - 1).isBlank()) {
      String last = parts.remove(parts.size() - 1);
      parts.set(parts.size() - 1, parts.get(parts.size() - 1) + "," + last);
    }
    return parts;
  }

  /** If a string, char literal or comment starts at {@code i}, return the index after it. */
  private static int skipNonCode(String s, int i) {
    int n = s.length();
    char c = s.charAt(i);
    if (c == '/' && i + 1 < n && s.charAt(i + 1) == '/') {
      int e = s.indexOf('\n', i);
      return e < 0 ? n : e;
    }
    if (c == '/' && i + 1 < n && s.charAt(i + 1) == '*') {
      int e = s.indexOf("*/", i + 2);
      return e < 0 ? n : e + 2;
    }
    if (c == '"') {
      int j = i + 1;
      while (j < n && s.charAt(j) != '"') j += s.charAt(j) == '\\' ? 2 : 1;
      return Math.min(j + 1, n);
    }
    if (c == '\'') {  // char literal like 'a' or '\n'; lifetimes like 'a are left alone
      if (i + 2 < n && s.charAt(i + 1) != '\\' && s.charAt(i + 2) == '\'') return i + 3;
      if (i + 3 < n && s.charAt(i + 1) == '\\') {
        int e = s.indexOf('\'', i + 2);
        if (e > 0 && e - i <= 10) return e + 1;
      }
    }
    return i;
  }
}
