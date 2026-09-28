package ai.serenade.treesitter;

/**
 * Wraps a tree-sitter TSNode (which is a 32-byte struct by value).
 * We store it as an opaque long pointer to a heap-allocated copy.
 */
public class Node {
  private final long ptr;  // pointer to heap-allocated TSNode

  Node(long ptr) { this.ptr = ptr; }

  public native int     getChildCount();
  public native Node    getChild(int i);
  public native String  getType();
  public native int     getStartByte();
  public native int     getEndByte();
  public native int     getStartRow();
  public native int     getStartColumn();
  public native boolean hasError();
}
