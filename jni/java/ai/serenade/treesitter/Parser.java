package ai.serenade.treesitter;

public class Parser implements AutoCloseable {
  static { TreeSitter.loadLib(); }

  private long ptr;

  public Parser() { ptr = createParser(); }

  private native long createParser();
  public  native void setLanguage(long lang);
  public  native Tree parseString(String src);

  @Override public void close() {
    if (ptr != 0) { destroyParser(ptr); ptr = 0; }
  }
  private native void destroyParser(long ptr);
}
