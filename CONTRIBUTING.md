# Contributing

Thanks for helping! Bug reports, browser/NAT compatibility notes, translations and UI polish are
as welcome as code.

## Setup

```bash
rustup toolchain install stable      # 1.85+
cargo test                           # no FreeSWITCH needed
cd web && npm ci && npm run build    # Node 22+
```

To make real calls you need FreeSWITCH. The quickest way is the Compose stack:

```bash
cp .env.example .env     # FSW_DOMAIN=localhost, FSW_FREESWITCH_SECRET=$(openssl rand -hex 32), FSW_EXT_IP=<your LAN IP>
make up                  # builds from source; open https://localhost
```

For UI work, run `make up` once for FreeSWITCH, then `docker compose stop fs-webrtc`,
`make run` (server on :8090) and `make web-dev` (hot reload on http://localhost:5173).
Open two browser profiles (or one normal and one private window) to call yourself.

## Before you open a PR

```bash
make fmt check      # cargo fmt, then fmt --check + clippy -D warnings + tests + web build
```

* New behaviour comes with a test. API changes come with updated `#[utoipa::path]` docs: the
  OpenAPI spec is generated from them.
* Anything that reaches FreeSWITCH (XML, dial strings) must go through `valid_username` in
  `src/api/mod.rs`. Never interpolate display names or other free text there.
* Schema changes are a new file in `migrations/`; never edit a released migration.
* Keep dependencies few; explain any new one in the PR.
* Add your change under `## [Unreleased]` in [CHANGELOG.md](CHANGELOG.md); maintainers turn
  that section into the release notes (see "Releases" in the README).

Commits follow [Conventional Commits](https://www.conventionalcommits.org) (`feat:`, `fix:`, `docs:` …).
