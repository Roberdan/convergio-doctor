# Changelog

## [0.1.11](https://github.com/Roberdan/convergio-doctor/compare/v0.1.10...v0.1.11) (2026-04-21)


### Features

* **doctor:** add check_real_world module with 5 production-blind checks ([fda8b5c](https://github.com/Roberdan/convergio-doctor/commit/fda8b5c7cfdace77782e63d612858107b44b4e56))
* **smoke:** cover all 81 capability descriptor routes ([6a7c381](https://github.com/Roberdan/convergio-doctor/commit/6a7c381209445aba630900f5dec69cc2ccfb018f))


### Bug Fixes

* **beta+e2e:** mesh checks compare versions correctly + resolve peer IPs ([e9c9ac6](https://github.com/Roberdan/convergio-doctor/commit/e9c9ac601b17679052c1f4526b5383ec7e1b8215))
* **ci:** remove unknown extra-artifacts input + cross-repo test step ([0925ed0](https://github.com/Roberdan/convergio-doctor/commit/0925ed09342badafdbe8bf94804188427210c649))
* **doctor:** compare cli vs daemon via subprocess, not crate CARGO_PKG_VERSION ([db0f9f0](https://github.com/Roberdan/convergio-doctor/commit/db0f9f0561ca0cee9f0c07007e4948ef8e3d2292))
* **doctor:** e2e CRUD + spawn checks use correct endpoints/filters ([16955fa](https://github.com/Roberdan/convergio-doctor/commit/16955fa2ea02cea753cc698a6516685c1ee7b9fa))
* **doctor:** fill ADR-036 + getting-started stubs with required content ([979720c](https://github.com/Roberdan/convergio-doctor/commit/979720c88f072bf02c379cd0cc16baace07bff91))
* **e2e/mesh:** fit sync poll inside daemon's 30s request timeout ([dd967e7](https://github.com/Roberdan/convergio-doctor/commit/dd967e7d78c39d5ac8879f3ab164b25e4cbd35ed))
* **e2e/mesh:** poll sync roundtrip for full cycle + use ssh_alias ([fd0fb5c](https://github.com/Roberdan/convergio-doctor/commit/fd0fb5ca7f0acae010b84edd8ea2353fdc02c076))
* **e2e/mesh:** prefer tailscale_ip over lan_ip when resolving peers ([47d707e](https://github.com/Roberdan/convergio-doctor/commit/47d707e2c720b4cce2ce7dfda726e06fce765563))
* **e2e/security:** actually replace is_dev_mode body (previous commit only bumped version) ([ef111a1](https://github.com/Roberdan/convergio-doctor/commit/ef111a1856dcbf14ac0a78d14ce5b2c8991984b5))
* **e2e/security:** force non-localhost path so auth checks actually run ([c075feb](https://github.com/Roberdan/convergio-doctor/commit/c075febdd149945d12f408e9ab548ceeb9922c8d))
* **e2e/security:** read dev_mode from /api/health, not from probe behavior ([a91f618](https://github.com/Roberdan/convergio-doctor/commit/a91f618431d07f2eb8e71dd04f6633897a0ca0e8))

## [0.1.10](https://github.com/Roberdan/convergio-doctor/compare/v0.1.9...v0.1.10) (2026-04-15)


### Bug Fixes

* cli_version_match compares against running daemon, not doctor crate version ([01bd991](https://github.com/Roberdan/convergio-doctor/commit/01bd9916ea0dd0969850de1e562783ed26eaf277))

## [0.1.9](https://github.com/Roberdan/convergio-doctor/compare/v0.1.8...v0.1.9) (2026-04-14)


### Bug Fixes

* pass CARGO_REGISTRY_TOKEN to release workflow ([d85f3a2](https://github.com/Roberdan/convergio-doctor/commit/d85f3a266a4809442a55e0e6f75735ddc3685742))

## [0.1.8](https://github.com/Roberdan/convergio-doctor/compare/v0.1.7...v0.1.8) (2026-04-13)


### Bug Fixes

* update Cargo.lock for crates.io deps ([42c7506](https://github.com/Roberdan/convergio-doctor/commit/42c75068fadc47fa46d87caa2e7c4db734201f84))

## [0.1.7](https://github.com/Roberdan/convergio-doctor/compare/v0.1.6...v0.1.7) (2026-04-13)


### Features

* adapt convergio-doctor for standalone repo ([25bbd5e](https://github.com/Roberdan/convergio-doctor/commit/25bbd5e5bc03f3dacd80d349e4734cca88606570))


### Bug Fixes

* bump convergio-depgraph v0.1.4 to align SDK types with v0.1.9 ([b57447c](https://github.com/Roberdan/convergio-doctor/commit/b57447c563eebde1cfff010d535a435fb1872f54))
* **release:** use vX.Y.Z tag format (remove component) ([b9caa73](https://github.com/Roberdan/convergio-doctor/commit/b9caa7378341d75278c048dfadc1a0a864399479))
* **security:** harden doctor against injection, SSRF, traversal, and secret exposure ([#4](https://github.com/Roberdan/convergio-doctor/issues/4)) ([38e44c2](https://github.com/Roberdan/convergio-doctor/commit/38e44c2424a69e6b29918f2ffa8792743e99b31f))


### Documentation

* add .env.example with required environment variables ([#6](https://github.com/Roberdan/convergio-doctor/issues/6)) ([e6fcb32](https://github.com/Roberdan/convergio-doctor/commit/e6fcb324f45099478af2d3c1b3a022184a2cbdc9))

## [0.1.5](https://github.com/Roberdan/convergio-doctor/compare/v0.1.4...v0.1.5) (2026-04-13)


### Features

* adapt convergio-doctor for standalone repo ([25bbd5e](https://github.com/Roberdan/convergio-doctor/commit/25bbd5e5bc03f3dacd80d349e4734cca88606570))


### Bug Fixes

* bump convergio-depgraph v0.1.4 to align SDK types with v0.1.9 ([b57447c](https://github.com/Roberdan/convergio-doctor/commit/b57447c563eebde1cfff010d535a435fb1872f54))
* **release:** use vX.Y.Z tag format (remove component) ([b9caa73](https://github.com/Roberdan/convergio-doctor/commit/b9caa7378341d75278c048dfadc1a0a864399479))
* **security:** harden doctor against injection, SSRF, traversal, and secret exposure ([#4](https://github.com/Roberdan/convergio-doctor/issues/4)) ([38e44c2](https://github.com/Roberdan/convergio-doctor/commit/38e44c2424a69e6b29918f2ffa8792743e99b31f))


### Documentation

* add .env.example with required environment variables ([#6](https://github.com/Roberdan/convergio-doctor/issues/6)) ([e6fcb32](https://github.com/Roberdan/convergio-doctor/commit/e6fcb324f45099478af2d3c1b3a022184a2cbdc9))

## [0.1.4](https://github.com/Roberdan/convergio-doctor/compare/convergio-doctor-v0.1.3...convergio-doctor-v0.1.4) (2026-04-12)


### Bug Fixes

* bump convergio-depgraph v0.1.4 to align SDK types with v0.1.9 ([b57447c](https://github.com/Roberdan/convergio-doctor/commit/b57447c563eebde1cfff010d535a435fb1872f54))

## [0.1.3](https://github.com/Roberdan/convergio-doctor/compare/convergio-doctor-v0.1.2...convergio-doctor-v0.1.3) (2026-04-12)


### Documentation

* add .env.example with required environment variables ([#6](https://github.com/Roberdan/convergio-doctor/issues/6)) ([e6fcb32](https://github.com/Roberdan/convergio-doctor/commit/e6fcb324f45099478af2d3c1b3a022184a2cbdc9))

## [0.1.2](https://github.com/Roberdan/convergio-doctor/compare/convergio-doctor-v0.1.1...convergio-doctor-v0.1.2) (2026-04-12)


### Bug Fixes

* **security:** harden doctor against injection, SSRF, traversal, and secret exposure ([#4](https://github.com/Roberdan/convergio-doctor/issues/4)) ([38e44c2](https://github.com/Roberdan/convergio-doctor/commit/38e44c2424a69e6b29918f2ffa8792743e99b31f))

## [0.1.1](https://github.com/Roberdan/convergio-doctor/compare/convergio-doctor-v0.1.0...convergio-doctor-v0.1.1) (2026-04-12)


### Features

* adapt convergio-doctor for standalone repo ([25bbd5e](https://github.com/Roberdan/convergio-doctor/commit/25bbd5e5bc03f3dacd80d349e4734cca88606570))

## 0.1.0 (Initial Release)

### Features

- Initial extraction from convergio monorepo
