package ai.serenade.treesitter;

public class Languages {
  static { TreeSitter.loadLib(); }
  public static native long rust();
}
