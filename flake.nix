{
  description = "Kiln - WebAssembly interpreter and runtime for safety-critical systems";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachSystem [
      "x86_64-linux"
      "aarch64-linux"
      "x86_64-darwin"
      "aarch64-darwin"
    ] (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        # Pinned Rust toolchain. Kept in sync with rust-toolchain.toml and
        # tool-versions.toml (stable 1.86.0; edition 2024 needs >= 1.85).
        # rustc-dev/llvm-tools are included for clippy, coverage (llvm-cov),
        # and rust-analyzer.
        rustToolchain = pkgs.rust-bin.stable."1.86.0".default.override {
          extensions = [ "rust-src" "clippy" "rustfmt" "llvm-tools-preview" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            # Pinned Rust toolchain (1.86.0 + clippy/rustfmt/rust-src)
            rustToolchain

            # WebAssembly tooling — differential testing against wasmtime and
            # component/module validation with wasm-tools.
            pkgs.wasmtime
            pkgs.wasm-tools

            # Release/supply-chain tooling (matches .github/workflows/release.yml):
            # CycloneDX SBOM generation and Sigstore signature verification.
            pkgs.cargo-cyclonedx
            pkgs.cosign

            # Traceability + fuzzing used by cargo-kiln workflows.
            pkgs.cargo-fuzz

            # Native build dependencies.
            pkgs.pkg-config
            pkgs.openssl
            pkgs.git
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            pkgs.darwin.apple_sdk.frameworks.Security
            pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
            pkgs.libiconv
          ];

          env = {
            RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          };

          shellHook = ''
            echo "kiln dev shell (reproducible toolchain via Nix)"
            echo "  rust:    $(rustc --version)"
            echo "  cargo:   $(cargo --version)"
            echo "  wasmtime: $(wasmtime --version 2>/dev/null)"
            echo "  cosign:  $(cosign version 2>/dev/null | head -1)"
            echo ""
            echo "  build the unified tool with: cargo install --path cargo-kiln"
          '';
        };

        # `nix build` produces the kilnd runtime CLI. Tests are skipped here —
        # they require the external/testsuite submodule and WAST fixtures that
        # are not part of the pure source closure.
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "kilnd";
          version = "0.3.1";
          src = pkgs.lib.cleanSource ./.;
          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = [ pkgs.openssl ]
            ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.darwin.apple_sdk.frameworks.Security
              pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
              pkgs.libiconv
            ];

          # Only build the kilnd CLI binary.
          cargoBuildFlags = [ "--package" "kilnd" ];
          doCheck = false;

          meta = {
            description = "WebAssembly interpreter and runtime for safety-critical systems";
            homepage = "https://github.com/pulseengine/kiln";
            license = pkgs.lib.licenses.mit;
            mainProgram = "kilnd";
          };
        };
      }
    );
}
