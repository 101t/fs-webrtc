# Security policy

fs-webrtc carries people's private conversations, so we treat security reports as top priority.

## Reporting

Please **do not open a public issue**. Use GitHub's
[private vulnerability reporting](https://github.com/101t/fs-webrtc/security/advisories/new).
Include the version, configuration (secrets removed) and steps to reproduce. You will get an
answer within a week.

## Supported versions

Only the latest 1.x release receives fixes.

## What is protected, and what is not

* Media is encrypted between each browser and your server (DTLS-SRTP) and relayed by FreeSWITCH,
  so participants never learn each other's IP address. It is **not end-to-end encrypted**:
  whoever controls the server can access calls. Run it on a server you trust.
* SIP signalling runs over the same HTTPS connection as the app (WebSocket at `/sip`).
* Only accepted, mutual contacts can call each other; FreeSWITCH asks the server on every call.

## Deployment checklist

- [ ] Served over HTTPS (the Compose stack does this with Caddy). Browsers refuse camera and microphone on plain HTTP.
- [ ] `FSW_FREESWITCH_SECRET` generated with `openssl rand -hex 32` (`install.sh` does this) and `.env` readable only by root.
- [ ] Port 8090 (fs-webrtc) and 5066/5090/8031 (FreeSWITCH) not reachable from the internet; the Compose stack binds them to 127.0.0.1.
- [ ] `FSW_SIGNUP=false` or `FSW_INVITE_CODE` set on a server reachable by strangers.
- [ ] Only TCP 80/443 and UDP 16384-16584 open in the firewall.
