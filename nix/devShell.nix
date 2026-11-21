pkgs:
pkgs.mkShell rec {
  buildInputs = with pkgs; [
    pkg-config
    fontconfig

    libxcb
    libxkbcommon
    libX11
    libXcursor
    libXi
    libXrandr
    libXxf86vm

    wayland
    wayland-protocols
    wayland-scanner
    vulkan-loader
  ];

  packages = with pkgs; [
    mold

    rustToolchain
    cargo-binstall

    just
    just-lsp

    quint
    temurin-bin
    buf

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
