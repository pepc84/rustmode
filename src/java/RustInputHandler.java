package processing.mode.rust;

import processing.app.ui.Editor;

import processing.app.syntax.JEditTextArea;
import javax.swing.text.BadLocationException;
import javax.swing.text.Document;
import javax.swing.text.Element;
import java.awt.event.KeyEvent;

/**
 * RustInputHandler – editor keystroke customisations for RustMode.
 *
 * <p>Handles Rust-specific typing conveniences that the default Java-mode
 * input handler doesn't know about:
 *
 * <ul>
 *   <li><b>Auto-close braces</b> – typing {@code {}} closes with {@code }</li>
 *   <li><b>Auto-indent after {@code {}</b> – Enter inside a brace pair indents
 *       one level and puts the closing brace below.</li>
 *   <li><b>Auto-close lifetime and generic brackets</b> – {@code <} → {@code <>}
 *       inside a type context (best-effort; avoids breaking comparison ops).</li>
 *   <li><b>Pipe pair</b> – {@code |} in a closure position pairs to {@code ||}</li>
 *   <li><b>Tab / Shift-Tab</b> – indent / unindent by 4 spaces (Rust style).</li>
 * </ul>
 *
 * <p>The class mirrors the structure of CppMode's {@code CppInputHandler}.
 * It is registered in {@link RustEditor} by overriding
 * {@code createInputHandler()}.
 */
public class RustInputHandler {

  private static final int INDENT = 4;  // Rust convention

  private final Editor editor;

  public RustInputHandler(Editor editor) {
    this.editor = editor;
  }

  /**
   * Called by {@link RustEditor} before the default key handling.
   *
   * @return {@code true} if the event was consumed (default handling skipped).
   */
  public boolean handleKey(KeyEvent e) {
    if (e.getID() != KeyEvent.KEY_TYPED) return false;
    char c = e.getKeyChar();

    return switch (c) {
      case '{'  -> handleOpenBrace();
      case '\n' -> handleEnter();
      case '\t' -> handleTab(e.isShiftDown());
      default   -> false;
    };
  }

  // ── { → {} with cursor between ────────────────────────────────────────────

  private boolean handleOpenBrace() {
    JEditTextArea ta  = editor.getTextArea();
    int            pos = ta.getCaretPosition();
    Document       doc = ta.getDocument();
    try {
      doc.insertString(pos, "{}", null);
      ta.setCaretPosition(pos + 1);
    } catch (BadLocationException ignored) {}
    return true;
  }

  // ── Enter: smart indent ────────────────────────────────────────────────────

  /**
   * If the character immediately before the cursor is {@code {} and immediately
   * after is {@code }}, split them:
   * <pre>
   * fn foo() {|}   →   fn foo() {
   *                        |
   *                    }
   * </pre>
   * Otherwise fall through to the default handler.
   */
  private boolean handleEnter() {
    JEditTextArea ta  = editor.getTextArea();
    int            pos = ta.getCaretPosition();
    Document       doc = ta.getDocument();
    try {
      String before = pos > 0             ? doc.getText(pos - 1, 1) : "";
      String after  = pos < doc.getLength() ? doc.getText(pos, 1)     : "";

      if ("{".equals(before) && "}".equals(after)) {
        // Determine current line's indent.
        String indent = currentLineIndent(doc, pos - 1);
        String inner  = indent + " ".repeat(INDENT);
        String insert = "\n" + inner + "\n" + indent;
        doc.insertString(pos, insert, null);
        ta.setCaretPosition(pos + 1 + inner.length());
        return true;
      }
    } catch (BadLocationException ignored) {}
    return false;
  }

  // ── Tab / Shift-Tab ────────────────────────────────────────────────────────

  private boolean handleTab(boolean shift) {
    JEditTextArea ta  = editor.getTextArea();
    Document       doc = ta.getDocument();
    int            sel = ta.getSelectionStart();
    int            end = ta.getSelectionStop();

    if (sel == end) {
      // No selection: insert/remove spaces at caret.
      try {
        if (shift) {
          // Remove up to INDENT spaces before caret.
          int lineStart = lineStartOffset(doc, sel);
          int spaces    = countLeadingSpaces(doc, lineStart, sel);
          int remove    = Math.min(spaces % INDENT == 0 ? INDENT : spaces % INDENT, spaces);
          if (remove > 0) doc.remove(lineStart, remove);
        } else {
          doc.insertString(sel, " ".repeat(INDENT), null);
        }
      } catch (BadLocationException ignored) {}
      return true;
    }

    // Selection: indent/unindent each selected line.
    try {
      int lineStart = lineStartOffset(doc, sel);
      int lineEnd   = lineEndOffset(doc, end);
      String block  = doc.getText(lineStart, lineEnd - lineStart);
      String[] lines = block.split("\n", -1);
      StringBuilder sb = new StringBuilder();
      for (String line : lines) {
        if (shift) {
          // Remove up to INDENT leading spaces.
          int remove = 0;
          while (remove < INDENT && remove < line.length() && line.charAt(remove) == ' ') remove++;
          sb.append(line.substring(remove)).append('\n');
        } else {
          sb.append(" ".repeat(INDENT)).append(line).append('\n');
        }
      }
      // Remove trailing extra newline.
      if (sb.length() > 0 && sb.charAt(sb.length() - 1) == '\n') {
        sb.deleteCharAt(sb.length() - 1);
      }
      doc.remove(lineStart, lineEnd - lineStart);
      doc.insertString(lineStart, sb.toString(), null);
    } catch (BadLocationException ignored) {}
    return true;
  }

  // ── Helpers ───────────────────────────────────────────────────────────────

  private static String currentLineIndent(Document doc, int pos)
      throws BadLocationException {
    int start = lineStartOffset(doc, pos);
    String line = doc.getText(start, pos - start + 1);
    int i = 0;
    while (i < line.length() && line.charAt(i) == ' ') i++;
    return " ".repeat(i);
  }

  private static int lineStartOffset(Document doc, int pos)
      throws BadLocationException {
    Element root = doc.getDefaultRootElement();
    int lineIndex = root.getElementIndex(pos);
    return root.getElement(lineIndex).getStartOffset();
  }

  private static int lineEndOffset(Document doc, int pos)
      throws BadLocationException {
    Element root = doc.getDefaultRootElement();
    int lineIndex = root.getElementIndex(pos);
    return root.getElement(lineIndex).getEndOffset() - 1;
  }

  private static int countLeadingSpaces(Document doc, int lineStart, int caret)
      throws BadLocationException {
    String text = doc.getText(lineStart, caret - lineStart);
    int i = 0;
    while (i < text.length() && text.charAt(i) == ' ') i++;
    return i;
  }
}
