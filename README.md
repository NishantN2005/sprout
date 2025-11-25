# Sprout

Sprout is a small educational compiler project written in Rust designed around **automatic partitioning and GPU acceleration**. 
The core vision is to build a compiler that automatically parallelizes code and offloads computations to GPUs wherever safe and beneficial, 
without requiring explicit parallelization directives from the programmer.

It contains a simple front-end (lexer + parser), a middle IR with lowering and basic optimizations, and a backend based on LLVM (via `inkwell`) 
for JIT-running generated code.

## Design Goals: Automatic Parallelization & GPU Offloading

Sprout is being designed with **automatic GPU partitioning** as a core principle. The compiler will:

- **Analyze data dependencies** to identify parallelizable regions of code.
- **Automatically partition work** across GPU threads and blocks where the compiler can guarantee correctness and safety.
- **Insert data transfer and synchronization** code transparently so the programmer sees sequential semantics.
- **Use ML/LLM-guided heuristics** (experimental) to make intelligent decisions about when and where to parallelize, balancing compute intensity, memory bandwidth, and synchronization overhead.

### Current Status

This is a **work-in-progress research compiler**. Currently implemented:
- Basic expression evaluation and variable assignment.
- Simple control flow (if statements without else).
- Constant folding optimization.
- LLVM-based JIT execution.

Future roadmap (targeting automatic parallelization):
- Loop analysis and vectorization hints.
- Memory aliasing analysis.
- Data layout and cache-aware optimizations.
- GPU code generation (PTX / SPIR-V).
- ML-guided cost models for partitioning decisions.
- Runtime scheduler for dynamic workload distribution.

This README explains how to build, run, and develop locally.

## Repository layout

- `src/frontend` - lexer and parser that produce ASTs
- `src/middle`  - lowering from AST to IR and optimization passes
- `src/backend` - LLVM codegen / JIT using `inkwell`
- `tests/`       - example source files used by the test runner in `src/main.rs`

## Building

This project uses `inkwell` / `llvm-sys` for the LLVM backend. That requires a
compatible LLVM installation on your system. On macOS you can install LLVM via Homebrew:

```bash
brew install llvm
```

After installing, set the environment variable that `llvm-sys` expects. For example:

```bash
export LLVM_SYS_211_PREFIX="$(brew --prefix llvm)"
```

Note: the exact `LLVM_SYS_<MAJOR>_PREFIX` name depends on the `llvm-sys` major
version in `Cargo.toml`. If you hit a message like "No suitable version of LLVM was
found...", check the `llvm-sys` error and set the prefix variable it mentions.

Alternatively, use `llvmenv` to install and switch LLVM versions (see the
`llvm-sys` documentation).

After LLVM is available, build normally:

```bash
cargo build
```

To run the test harness (the CLI reads files from `tests/`):

```bash
cargo run
```

## Running without LLVM (development)

If you are working on the front-end or middle-end and want to iterate without
installing LLVM, you can temporarily stub or gate the backend. Two options:

- Add a Cargo feature that disables the `inkwell` backend and provides a small
  stub backend (returns the lowered IR or prints it). This requires editing
  `Cargo.toml` and `src/backend/mod.rs`.
- Or run unit tests that only exercise lowering/optimization passes (no JIT).

## Tests (examples)

Example test file `tests/unary.sp` is included. The `main` driver reads `tests/`
and runs the parser, the lowering, optional optimization, then JIT (if LLVM is
available).

You can add more `.sp` files to `tests/` to exercise language features. Each
file can contain multiple statements; the parser expects statements to be
terminated (e.g., with `;` or newline depending on your lexer).

## Development notes

- Lowering: `src/middle/lower.rs` maps AST -> IR. For assignment expressions we
  currently lower `x = expr` by evaluating `expr`, emitting a `Store` to the
  variable, then emitting a `Load` to produce a ValueId that the rest of the IR
  can reference.

- Optimizations: simple passes live in `src/middle/opt.rs`. Currently a
  constant-folding pass is available. More passes (peephole, DCE, store-load
  elimination) can be added and composed via `optimize_module`.

- Backend: `src/backend/llvm.rs` contains LLVM IR generation and JIT execution.
  If you see panics like `Found PointerValue but expected IntValue`, it usually
  means a `build_load` was called with the wrong type overload; check that the
  builder loads the element type (not a `ptr` type) or use the pointer-only
  overload `build_load(ptr, name)`.

