# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Changed

- Replace curl shell-out with official `cloudflare` crate (v0.14) for
  Cloudflare DNS provider, removing external curl dependency and adding
  typed API calls with proper error handling ([5611350], [#11])

## [0.4.0] - 2026-02-25

### Added

- `App::port(host, container)` builder for host-to-container port mapping
  (docker-compose `ports`), complementing `expose()` which only exposes
  within the Docker network ([893d9b9], [#8])
- `--force` flag on `destroy` command to skip interactive confirmation
  prompt for CI/CD and scripted workflows ([e883ebe], [#10])

### Fixed

- Caddy now only depends on apps it actually proxies, not all apps in the
  stack; non-proxied apps no longer block Caddy startup ([af6dc66], [#9])

### Changed

- **BREAKING**: `Caddy::reverse_proxy()` and `Caddy::route()` now require
  `Upstream` instead of `impl Into<String>`, callers must use
  `App::upstream()` ([ca10728])
- Remove `From<Upstream> for String` impl ([ca10728])
- `Pipeline` now supports multiple DNS providers; calling `.dns()` pushes
  to a vec instead of replacing the previous provider ([1ea20e5], [#7])

## [0.3.0] - 2026-02-24

### Added

- Libvirt/KVM provisioner for home lab deployments with bridged and NAT
  networking ([abdd0ce], [#5])
- `NetworkMode` enum for choosing bridged or NAT VM networking ([abdd0ce], [#5])
- `detect_ssh_key` default method on `Provisioner` trait ([abdd0ce], [#5])
- Shared SSH config helpers extracted to `provision` module ([abdd0ce], [#5])
- Beginner-friendly rustdoc guide for home lab setup with libvirt/KVM
  ([abdd0ce], [#5])
- Libvirt deployment example in `examples/libvirt_deploy.rs` ([abdd0ce], [#5])
- `Upstream` struct for type-safe Caddy reverse proxy references
  ([f90cfa7])
- `App::upstream()` and `App::upstream_port()` to derive upstream
  from exposed ports ([f90cfa7])
- `Caddy::volume()` builder for mounting custom volumes into the Caddy
  container ([5d27688], [#6])

### Changed

- `Caddy::reverse_proxy()` and `Caddy::route()` now accept
  `impl Into<String>` instead of `&str` ([f90cfa7])
- `cmd_provision` now calls generic `detect_ssh_key()` instead of
  DO-specific function ([abdd0ce], [#5])

### Infrastructure

- Include tests in published crate to fix `cargo package` warnings
  ([973cf31])

## [0.2.0] - 2026-02-24

### Added

- Support multiple apps in a single Pipeline with path-based Caddy routing
  ([07e32cf], [#4])
- `Caddy::route()` builder for `handle` block path routing ([07e32cf], [#4])
- `Caddy::has_upstreams()` helper method ([07e32cf], [#4])
- `Pipeline::multi()` constructor for multi-app deployments ([07e32cf], [#4])
- Multi-app example in `examples/multi_app.rs` ([07e32cf], [#4])
- `--dry-run` flag to preview generated files without deploying ([c25734f])
- `status` subcommand to show container status on remote server ([c25734f])
- Healthcheck polling instead of fixed sleep during deploy ([c25734f])
- Image size logging and progress bar (via `pv`) during transfer ([bbaf467])
- All examples rendered in rustdoc under Architecture section ([e7d6469])
- Repository, docs, and crates.io links in rustdoc header ([837af24])

### Changed

- `compose::render()` now accepts `&[App]` instead of `&App` ([07e32cf], [#4])
- `Deployer::deploy()` now accepts `&[App]` instead of `&App` ([07e32cf], [#4])
- `Provisioner::setup_server()` no longer requires `&App` and `&Caddy`
  params ([07e32cf], [#4])
- Caddy service in compose depends on all apps, not just one ([07e32cf], [#4])
- Network named after first app, shared by all services ([07e32cf], [#4])
- `.env` files transferred as `.env.{app.name}` for multi-app setups
  ([07e32cf], [#4])

### Infrastructure

- Extracted server setup to bash script ([c0747ef])
- Moved unit tests to integration tests ([4faf950])
- Fixed nightly rustfmt formatting ([ffb0790])

## [0.1.0] - 2025-06-01

### Added

- Initial release with declarative deployment DSL
- `App` builder for Docker container configuration
- `Caddy` builder for reverse proxy with TLS, gzip, security headers, basic
  auth
- `Pipeline` orchestrator with provision, DNS, and deploy phases
- DigitalOcean provisioner via `doctl` CLI
- OVH DNS provider via REST API
- Cloudflare DNS provider via API
- Docker save/load deployment strategy (no registry required)
- Caddyfile and docker-compose.yml generation
- SSH session management with key detection
- `provision`, `deploy`, `destroy` CLI subcommands

<!-- Commit links -->
[5611350]: https://github.com/LeakIX/catapulta/commit/5611350
[e883ebe]: https://github.com/LeakIX/catapulta/commit/e883ebe
[af6dc66]: https://github.com/LeakIX/catapulta/commit/af6dc66
[893d9b9]: https://github.com/LeakIX/catapulta/commit/893d9b9
[1ea20e5]: https://github.com/LeakIX/catapulta/commit/1ea20e5
[ca10728]: https://github.com/LeakIX/catapulta/commit/ca10728
[5d27688]: https://github.com/LeakIX/catapulta/commit/5d27688
[973cf31]: https://github.com/LeakIX/catapulta/commit/973cf31
[f90cfa7]: https://github.com/LeakIX/catapulta/commit/f90cfa7
[abdd0ce]: https://github.com/LeakIX/catapulta/commit/abdd0ce
[07e32cf]: https://github.com/LeakIX/catapulta/commit/07e32cf
[c25734f]: https://github.com/LeakIX/catapulta/commit/c25734f
[bbaf467]: https://github.com/LeakIX/catapulta/commit/bbaf467
[4faf950]: https://github.com/LeakIX/catapulta/commit/4faf950
[c0747ef]: https://github.com/LeakIX/catapulta/commit/c0747ef
[ffb0790]: https://github.com/LeakIX/catapulta/commit/ffb0790
[e7d6469]: https://github.com/LeakIX/catapulta/commit/e7d6469
[837af24]: https://github.com/LeakIX/catapulta/commit/837af24

<!-- PR/Issue links -->
[#11]: https://github.com/LeakIX/catapulta/issues/11
[#10]: https://github.com/LeakIX/catapulta/issues/10
[#9]: https://github.com/LeakIX/catapulta/issues/9
[#8]: https://github.com/LeakIX/catapulta/issues/8
[#7]: https://github.com/LeakIX/catapulta/issues/7
[#6]: https://github.com/LeakIX/catapulta/issues/6
[#5]: https://github.com/LeakIX/catapulta/issues/5
[#4]: https://github.com/LeakIX/catapulta/issues/4

<!-- Release links -->
[0.4.0]: https://github.com/LeakIX/catapulta/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/LeakIX/catapulta/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/LeakIX/catapulta/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/LeakIX/catapulta/releases/tag/v0.1.0
