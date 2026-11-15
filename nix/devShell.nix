pkgs:
pkgs.mkShell rec {
  buildInputs = with pkgs; [
    pkg-config
    fontconfig
    libxcb
    libxkbcommon

    wayland
    vulkan-loader
    libglvnd
  ];

  packages = with pkgs; [
    mold

    rustToolchain
    cargo-binstall

    just
    just-lsp

    quint
    temurin-bin

    nats-server
    nats-top
    natscli
  ];

  env = with pkgs; rec {
    LD_LIBRARY_PATH = lib.makeLibraryPath buildInputs;
    RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
  };

  shellHook = ''
    export CARGO_HOME="$PWD/.cargo"
    export PATH="$CARGO_HOME/bin:$PWD/target/debug:$PATH"
  '';
}
