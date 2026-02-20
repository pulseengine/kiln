# wrt-component

WebAssembly Component Model support for WRT.

## Current Status

**Early Development** - Basic component execution works:

- Component binary parsing
- Core module extraction from components
- Component instantiation
- WASI Preview 2 interface resolution

### In Progress

- Full component linking
- Cross-component calls
- Resource management

## Usage

Components are executed through `wrtd`:

```bash
wrtd my_component.wasm --component
```

## Component Model Support

Parses and executes WebAssembly components following the Component Model specification. Currently supports:

- Component sections (types, imports, exports, modules)
- Alias resolution
- Canon lift/lower for host calls
- WASI Preview 2 interface matching

## WASI Preview 2

Integrated support for WASI Preview 2 interfaces:

- `wasi:cli/stdout` - Standard output
- `wasi:cli/stderr` - Standard error
- `wasi:io/streams` - Stream operations

Additional interfaces in development.

## no_std Support

Works in `no_std` environments using bounded collections from `wrt-foundation`.

## License

MIT License
