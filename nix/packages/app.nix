{
  lib,
  stdenv,

  makeRustPlatform,
  rustToolchain,

  pkg-config,
  makeBinaryWrapper,

  fontconfig,
  libxcb,
  libxkbcommon,
  wayland,
  vulkan-loader,
  libglvnd,
}:

let
  workspace = (builtins.fromTOML (builtins.readFile ../../Cargo.toml)).workspace.package;
in
(makeRustPlatform {
  cargo = rustToolchain;
  rustc = rustToolchain;
}).buildRustPackage
  rec {
    pname = "beamlet-app";
    inherit (workspace) version;
    src = lib.fileset.toSource {
      root = ../..;
      fileset = lib.fileset.unions [
        ../../Cargo.toml
        ../../Cargo.lock
        ../../apps
        ../../crates
      ];
    };
    cargoLock.lockFile = ../../Cargo.lock;
    cargoBuildFlags = [ "--package=beamlet-app" ];
    cargoTestFlags = [ "--package=beamlet-app" ];

    nativeBuildInputs = [
      pkg-config
      makeBinaryWrapper
    ];

    buildInputs = lib.optionals stdenv.hostPlatform.isLinux [
      fontconfig
      libxcb
      libxkbcommon
      wayland
      vulkan-loader
      libglvnd
    ];

    postFixup = lib.optionalString stdenv.hostPlatform.isLinux ''
      wrapProgram "$out/bin/beamlet-app" \
        --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath buildInputs}
    '';

    meta = {
      description = "Beamlet app";
      homepage = workspace.repository;
      license = lib.licenses.asl20;
      mainProgram = "beamlet-app";
      platforms = lib.platforms.linux;
    };
  }
