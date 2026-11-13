final: prev: {
  rustToolchain =
    let
      rust = prev.rust-bin;
    in
    if builtins.pathExists ../rust-toolchain.toml then
      rust.fromRustupToolchainFile ../rust-toolchain.toml

    else if builtins.pathExists ../rust-toolchain then
      rust.fromRustupToolchainFile ../rust-toolchain

    else
      rust.stable."1.100.0".default.override {
        extensions = [
          "rust-src"
          "rustfmt"
          "rust-analyzer"
          "clippy"
          "cargo"
          "llvm-tools"
        ];

        targets = [
          "wasm32-wasip2"
        ];
      };
}
