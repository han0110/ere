<p align="center">
  <img src="assets/logo-blue-white.svg" alt="Ere" width="300"/>
</p>

<h1 align="center">Ere - Unified zkVM Interface and Toolkit </h1>

## Overview

**ere** is a Rust workspace providing a unified interface and toolkit for working with multiple zero-knowledge virtual machines (zkVMs). It abstracts over the differences between zkVMs, allowing you to compile, execute, prove, and verify programs across several backends with a common API.

Supported zkVMs:

- **SP1**
- **OpenVM**
- **RISC Zero**

Partially supported zkVMs:

- **Jolt** (Prover incompatibility)
- **Pico** (Missing execute and verify methods in API)
- **Zisk** (Docker support)

## Features

- **Unified Rust API** for compiling, executing, proving, and verifying zkVM programs
- **Pluggable backends**: swap between zkVMs with minimal code changes
- **Tests**: each backend has dedicated guest programs and integration tests
- **SDK installer scripts** for all supported zkVMs

## Directory Structure

- `crates/zkvm-interface/` — Core traits (`Compiler`, `zkVM`), input serialization, and reporting types
- `crates/ere-sp1/` — SP1 zkVM backend
- `crates/ere-jolt/` — Jolt zkVM backend
- `crates/ere-pico/` — Pico zkVM backend
- `crates/ere-openvm/` — OpenVM backend
- `crates/ere-risczero/` — RISC Zero backend
- `tests/` — Example guest programs and integration tests for each backend
- `scripts/sdk_installers/` — Shell scripts to install the SDKs for each zkVM
- `docker/` — Dockerfiles and build contexts for each zkVM environment

## How It Works

### 1. The Interface

The `zkvm-interface` crate defines two main traits:

- **Compiler**: Compiles a guest program (e.g., a Rust crate) into the appropriate binary/artifact for a zkVM
- **zkVM**: Executes, proves, and verifies programs on a zkVM

Each backend implements these traits for its own types.

### 2. Backend Crates

Each backend crate (e.g., `ere-sp1`, `ere-jolt`) implements the `Compiler` and `zkVM` traits, handling the specifics of compilation, execution, proof generation, and verification for its respective zkVM. The `Compiler` and `zkVm` trait living in the same crate is purely coincidental, it is entirely possible for the compiler to live elsewhere.

### 3. Input Handling

The `Input` struct in `zkvm-interface` serializes input data for guest programs, supporting chunked and contiguous access. Some zkVMs will ask for the input as a list of lists of bytes, while others want a list of bytes.

### 4. Testing

Each backend has a set of guest programs and tests under `tests/<backend>/`, which are used to validate compilation, execution, and proof/verification flows. This test corpus is expected to grow over time.

### 5. SDK Installation

SDKs for each zkVM can be installed using the scripts in `scripts/sdk_installers/`.

## Usage

### 1. Install SDKs

Before using a backend, install its SDK:

```sh
bash scripts/sdk_installers/install_sp1_sdk.sh
bash scripts/sdk_installers/install_jolt_sdk.sh
```

### 2. Add as a Dependency

Add the relevant crates to your `Cargo.toml` as needed.

### 3. Example: Compiling and Proving a Program

```rust
use zkvm_interface::{Compiler, zkVM, Input};
use ere_sp1::{EreSP1, RV32_IM_SUCCINCT_ZKVM_ELF};

let program_path = std::path::Path::new("path/to/guest");
let elf_bytes = RV32_IM_SUCCINCT_ZKVM_ELF::compile(program_path).unwrap();

let mut input = Input::new();
input.write(&42u32).unwrap();

let (proof, report) = EreSP1::prove(&elf_bytes, &input).unwrap();
EreSP1::verify(&elf_bytes, &proof).unwrap();
```

### 4. Running Tests

Each backend crate and guest program has its own tests. Run them with:

```sh
cargo test --workspace
```

> Note: for this to work, you will need to have installed the relevant toolchain and target for the zkVM test you want to run. The recommended workflow is to use Docker.

## Contributing

- Contributions are welcome! Please open issues or pull requests

## Disclaimer

zkVMs are rapidly improving, so the API is subject to a lot of change. In terms of scope, although the API is generic, the main use case will be zkEVMs. This may manifest itself in the selection of precompiles that
may be chosen as defaults.

## License

MIT OR Apache-2.0
