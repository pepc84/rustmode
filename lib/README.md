# natives/

Drop the tree-sitter-rust native grammar library for each platform here.
The filename must be exactly:

  linux-x86-64/   libtree-sitter-rust.so
  macos-arm64/    libtree-sitter-rust.dylib
  macos-x86-64/   libtree-sitter-rust.dylib
  windows-x86-64/ tree-sitter-rust.dll

Build from source:

  git clone https://github.com/tree-sitter/tree-sitter-rust
  cd tree-sitter-rust
  # Linux / macOS:
  gcc -shared -fPIC -o libtree-sitter-rust.so src/parser.c src/scanner.c -I./src
  # Windows (MSVC): cl /LD src/parser.c src/scanner.c /I src /Fe:tree-sitter-rust.dll

If CppMode already compiled tree-sitter-cpp, the same build system will
produce the Rust grammar - just swap the source directory.

RustMode loads the library at startup via Languages.rust() from the
serenade java-tree-sitter binding. The binding looks for the library on
java.library.path; the build.gradle bundles these into the jar so the IDE
can extract and load them automatically.
