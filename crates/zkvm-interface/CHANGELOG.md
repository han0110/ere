# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0](https://github.com/han0110/ere/releases/tag/v0.6.0) - 2026-03-26

### Added

- Add prover network parameter ([#35](https://github.com/han0110/ere/pull/35))
- Add `ere-zisk` with only compile and execute utility. ([#27](https://github.com/han0110/ere/pull/27))

### Fixed

- imports

### Other

- Update ZisK to `0.16.0` ([#296](https://github.com/han0110/ere/pull/296))
- Update SP1 to `v6.0.0` ([#294](https://github.com/han0110/ere/pull/294))
- Add `ProverResouce::Cluster` and add cluster support for ZisK ([#284](https://github.com/han0110/ere/pull/284))
- Support `--program-path` flag in `ere-server` ([#270](https://github.com/han0110/ere/pull/270))
- Revert the `untagged` derive ([#252](https://github.com/han0110/ere/pull/252))
- Patches for `zkboost` ([#250](https://github.com/han0110/ere/pull/250))
- Refactors ([#244](https://github.com/han0110/ere/pull/244))
- Refactor docs and make `Input` API more explicit ([#235](https://github.com/han0110/ere/pull/235))
- Input structure to support composition proof ([#230](https://github.com/han0110/ere/pull/230))
- Refactor crate `ere-io` ([#233](https://github.com/han0110/ere/pull/233))
- Add `print` and `cycle_scope` methods for `Platform` ([#224](https://github.com/han0110/ere/pull/224))
- Introduce `CompilerKind` and refactor `ere-dockerized` ([#221](https://github.com/han0110/ere/pull/221))
- Support `OutputHasher` for all platform crate ([#205](https://github.com/han0110/ere/pull/205))
- Add crates `ere-platform-{zkvm}` ([#202](https://github.com/han0110/ere/pull/202))
- Install rust src if not found ([#185](https://github.com/han0110/ere/pull/185))
- Refactor mods to separate compiler and zkvm ([#184](https://github.com/han0110/ere/pull/184))
- Alternative of #182 ([#183](https://github.com/han0110/ere/pull/183))
- Refactor `zkVM` error handling ([#179](https://github.com/han0110/ere/pull/179))
- `zkVM` takes opaque input ([#173](https://github.com/han0110/ere/pull/173))
- Reorganize crates ([#169](https://github.com/han0110/ere/pull/169))
- Support multiple proof kind ([#161](https://github.com/han0110/ere/pull/161))
- Refactor `ere-dockerized` ([#160](https://github.com/han0110/ere/pull/160))
- zkVK methods return public values ([#106](https://github.com/han0110/ere/pull/106))
- Risc0 check receipt variant ([#100](https://github.com/han0110/ere/pull/100))
- Add `InputItem::SerializedObject` ([#80](https://github.com/han0110/ere/pull/80))
- Refactor `ere-dockerized` ([#77](https://github.com/han0110/ere/pull/77))
- Add `ere-dockerized` ([#75](https://github.com/han0110/ere/pull/75))
- Add `ere-cli` ([#71](https://github.com/han0110/ere/pull/71))
- Add constructor function for `trait zkVM` ([#61](https://github.com/han0110/ere/pull/61))
- separate workspace and guest program directories ([#57](https://github.com/han0110/ere/pull/57))
- Support extra traits ([#55](https://github.com/han0110/ere/pull/55))
- Fix Pico's docker test ([#51](https://github.com/han0110/ere/pull/51))
- allow dynamic dispatch in zkVM trait ([#52](https://github.com/han0110/ere/pull/52))
- Add automatic name and sdk version ([#48](https://github.com/han0110/ere/pull/48))
- add execution duration
- move reports to reports.rs
- InputErased -> Input
- clippy
- remove old `Input`
- use InputErased
- expose InputItem
- expose InputErased
- use enum
- add initial code for inputErased
- concrete error
- add auto_impl
- move `new` out of zkvm-interface
- modify interface to add ProverResourceType
- Make reports serializable
- add method to write directly to slice
- make API stateful
- loosen bounds on zkinterface
- Program now is AsRef<[u8>]
- move Input to separate module
- split compiler from zkVM
- add zkVM interface crate
