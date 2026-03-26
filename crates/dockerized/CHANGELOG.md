# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0](https://github.com/han0110/ere/releases/tag/v0.6.0) - 2026-03-26

### Other

- Remove some zkVMs ([#311](https://github.com/han0110/ere/pull/311))
- Update ZisK to `0.16.0` ([#296](https://github.com/han0110/ere/pull/296))
- Remove some zkVMs ([#307](https://github.com/han0110/ere/pull/307))
- Update SP1 to `v6.0.0` ([#294](https://github.com/han0110/ere/pull/294))
- Build and publish cuda enabled docker image ([#291](https://github.com/han0110/ere/pull/291))
- Add `ProverResouce::Cluster` and add cluster support for ZisK ([#284](https://github.com/han0110/ere/pull/284))
- Updated ere-compiler to output ELF bytes and program digest separately ([#278](https://github.com/han0110/ere/pull/278))
- Fix docker image tag ([#274](https://github.com/han0110/ere/pull/274))
- Add script `build-image.sh` ([#273](https://github.com/han0110/ere/pull/273))
- Support `--program-path` flag in `ere-server` ([#270](https://github.com/han0110/ere/pull/270))
- Test ere dockerized complier produces deterministic/reproducible elfs ([#271](https://github.com/han0110/ere/pull/271))
- Manage `sp1-gpu` container by `ere-sp1` ([#263](https://github.com/han0110/ere/pull/263))
- support ERE_DOCKER_NETWORK ([#262](https://github.com/han0110/ere/pull/262))
- Refactor platform impls a bit ([#261](https://github.com/han0110/ere/pull/261))
- Support setting how many GPUs to expose in dockerized environment ([#258](https://github.com/han0110/ere/pull/258))
- Patches for `zkboost` ([#250](https://github.com/han0110/ere/pull/250))
- Refactors ([#244](https://github.com/han0110/ere/pull/244))
- Add 1 hour timeout for zisk proving ([#245](https://github.com/han0110/ere/pull/245))
- Update `zisk` to `v0.14.0` ([#220](https://github.com/han0110/ere/pull/220))
- Refactor docs and make `Input` API more explicit ([#235](https://github.com/han0110/ere/pull/235))
- Input structure to support composition proof ([#230](https://github.com/han0110/ere/pull/230))
- Refactor crate `ere-io` ([#233](https://github.com/han0110/ere/pull/233))
- Improve crates `ere-platform-{zkvm}` a bit ([#228](https://github.com/han0110/ere/pull/228))
- Retry RPC and restart server container when necessary ([#225](https://github.com/han0110/ere/pull/225))
- Introduce `CompilerKind` and refactor `ere-dockerized` ([#221](https://github.com/han0110/ere/pull/221))
- Remove default value of `RUSTFLAGS` ([#208](https://github.com/han0110/ere/pull/208))
- TamaGo compilation support ([#206](https://github.com/han0110/ere/pull/206))
- Make server handler non-blocking ([#204](https://github.com/han0110/ere/pull/204))
- Add crates `ere-platform-{zkvm}` ([#202](https://github.com/han0110/ere/pull/202))
- Update `zisk` to `v0.13.0` ([#187](https://github.com/han0110/ere/pull/187))
- Refactor mods to separate compiler and zkvm ([#184](https://github.com/han0110/ere/pull/184))
- Update Jolt to `v0.3.0-alpha` ([#181](https://github.com/han0110/ere/pull/181))
- Refactor `zkVM` error handling ([#179](https://github.com/han0110/ere/pull/179))
- Refactor dockerized zkvm error ([#176](https://github.com/han0110/ere/pull/176))
- Integrate Airbender ([#175](https://github.com/han0110/ere/pull/175))
- `zkVM` takes opaque input ([#173](https://github.com/han0110/ere/pull/173))
- Reorganize crates ([#169](https://github.com/han0110/ere/pull/169))
