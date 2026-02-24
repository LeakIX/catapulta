[![CI](https://github.com/LeakIX/catapulta/actions/workflows/ci.yaml/badge.svg)](https://github.com/LeakIX/catapulta/actions/workflows/ci.yaml)
[![crates.io](https://img.shields.io/crates/v/catapulta.svg)](https://crates.io/crates/catapulta)
[![docs](https://img.shields.io/badge/docs-github%20pages-blue)](https://leakix.github.io/catapulta/catapulta/)
[![license](https://img.shields.io/crates/l/catapulta)](LICENSE)

# catapulta

Portuguese for *catapult* - launch your application to any server.

**Full documentation:
[leakix.github.io/catapulta](https://leakix.github.io/catapulta/catapulta/)**

Declarative deployment DSL for Rust. Provision cloud servers,
configure DNS, and deploy Docker containers - all from typed Rust
code. No YAML, no shell scripts, no manual SSH.

## Supported providers

### Cloud provisioning

- **DigitalOcean** - create/destroy droplets via `doctl` CLI

### DNS

- **OVH** - A record management via OVH REST API
- **Cloudflare** - A record management via Cloudflare API

### Deployment

- **Docker save/load** - build locally, transfer via SSH
  (no registry required)

### Reverse proxy

- **Caddy** - automatic TLS, reverse proxy, gzip, security
  headers, basic auth

## License

MIT
