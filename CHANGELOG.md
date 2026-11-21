# Changelog

All notable changes are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow [SemVer](https://semver.org).
Each `## [x.y.z]` section becomes the notes of the GitHub Release for tag `vx.y.z`.

## [Unreleased]

## [1.0.0] - 2026-11-21

Complete rewrite. The PHP + SIP.js 0.13 demo with hard-coded test users is now a self-hosted
calling app.

### Added
- Accounts (argon2id passwords, 30-day sessions) with open, closed or invite-code sign-up.
- Mutual contacts: requests, accept, decline, remove. Only accepted contacts can call each other,
  enforced by FreeSWITCH's dialplan, not just the UI.
- One-to-one voice and video calls in the browser: ringing dialog, ringback, mute, camera
  toggle, call timer, missed/declined/busy notices, online indicator.
- Account settings: change password (signs out other devices, replaces the SIP password) and
  delete account.
- Sign-in protection: an account locks for 15 minutes after 5 wrong passwords.
- TURN servers with credentials (`FSW_TURN_USERNAME`, `FSW_TURN_PASSWORD`) for networks that block UDP.
- Rust server (axum, SQLite) that serves the React app, the REST API with OpenAPI docs at `/docs`,
  and relays SIP over WebSocket to FreeSWITCH at `/sip`.
- FreeSWITCH driven by mod_xml_curl: users and call routes come from the server; the shipped
  `freeswitch.xml` has no default users, passwords or PSTN access.
- React 19 + TypeScript + Vite web app using JsSIP (replacing the unmaintained SIP.js 0.13).
- Docker image, Docker Compose stack with FreeSWITCH and Caddy (automatic HTTPS), one-line
  `install.sh`, hardened systemd unit.
- CI, `Makefile` release targets and tag-driven releases: Docker image on GHCR
  (`X.Y.Z`, `X.Y`, `X`, `latest`), static Linux binary with checksum, GitHub Release.

### Removed
- PHP pages and the hard-coded `1000`/`1001` test extensions.
