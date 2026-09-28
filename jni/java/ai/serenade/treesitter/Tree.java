package ai.serenade.treesitter;

public class Tree implements AutoCloseable {
  private long ptr;

  Tree(long ptr) { this.ptr = ptr; }

  public native Node getRootNode();

  @Override public void close() {
    if (ptr != 0) { freeTree(ptr); ptr = 0; }
  }
  private native void freeTree(long ptr);
}
