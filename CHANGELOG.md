## [0.7.0] - 2026-03-26

### 🚀 Features

- Update zisk to 0.17.0 (test)
## [0.6.0] - 2026-03-26

### 🚀 Features

- Add `ere-zisk` with only compile and execute utility. (#27)
- Add prover network parameter (#35)
- Add serde for Risc0Program (#45)
- Impl zkVM for Nexus zkvm  (#47)
- `ere-miden`  (#136)
- Upgrade ziren & pico & miden (#247)
- *(nexus)* Add extensions support (#279)
- *(jolt)* Add secp256k1 inlines support  (#286)
- *(jolt)* Add keccak and sha2 inlines (#290)

### 🐛 Bug Fixes

- Copy host directory
- Imports
- Pin `openvm` version and upgrade the latest tag (#32)
- *(miden)* Register precompile verifiers (#288)
- *(jolt)* Update to rev 2e05fe88, enable ZK, fix no-std platform (#305)

### 💼 Other

- Fill in as much as possible for Pico zkvm methods
- Cache proving and verifying key
- Add execution duration
- Use compressed proofs
- Generate succinct proofs (#38)
- Update sdk (#40)
- Implement workaround for poisoned mutex when prover crashes
- Allow dynamic dispatch in zkVM trait (#52)
- Tempdir for `cargo-zisk rom-setup` (#39)
- Use Docker for guest program compilation (#54)
- Support extra traits (#55)
- Update sdk (#69)
- Compile sp1 guest program with stock rust compiler (#102)
- Guest program compilation with stock rust compiler. (#114)
- Guest program compilation with stock rust compiler. (#115)
- Guest program compilation with stock rust compiler. (#116)
- Guest program compilation with stock rust compiler. (#118)
- Allocator alignments test (#120)
- Add memlock unlimited (#145)
- Add clang dep (#164)
- Avoid blocking worker threads (#163)
- Implement execute and io serialization (#171)

### 🚜 Refactor

- Only do `rom-setup` when proving, and add `ZiskTempDir` in `ere-zisk` (#31)

### 📚 Documentation

- Separate workspace and guest program directories (#57)
- Support ERE_DOCKER_NETWORK (#262)

### ⚙️ Miscellaneous Tasks

- Add method to write directly to slice
- Add auto_impl
- Expose zkVm initializers
- Fix compiler docker tag (#162)
- Upgrade zkvm versions (#281)
