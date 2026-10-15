# kiln - WebAssembly runtime CLI

Command-line runner for WebAssembly core modules, with WASI Preview 2 host
support. `kiln <module.wasm>` runs a module and exits; it is not a daemon.

The binary was called `kilnd` up to v0.5.0. v0.6.0 ships `kilnd` alongside
`kiln` as a deprecated alias that behaves the same but prints a warning on
stderr; the alias is removed in v0.8.0.

## Components

kiln runs core modules. A Component Model component is lowered to a core
module at build time with [meld](https://github.com/pulseengine/meld) (RFC #46),
then run with kiln:

```bash
meld fuse hello_rust.wasm -o hello_rust.core.wasm
kiln hello_rust.core.wasm
```

Passing a component directly is refused with that instruction.

## Installation

```bash
cargo build -p kilnd --release
# binaries: target/release/kiln (and the deprecated target/release/kilnd)
```

## Usage

```bash
# Run a command module via its wasi:cli/run entry
kiln app.core.wasm

# Invoke a named export with core-scalar arguments and print its result
kiln --invoke add --arg 21 --arg 2 lib.core.wasm

# With resource limits
kiln --fuel 1000000 --memory 1048576 app.core.wasm

# Report the version
kiln --version
```

`kiln --help` lists every option. `--help` and `--version` exit 0; an unknown
flag exits 2 with usage on stderr.

## License

MIT License
