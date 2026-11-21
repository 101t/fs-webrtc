# Deployment guide

The [Quick start](../README.md#quick-start) covers the common case: one Linux server with Docker,
a domain name, and the Compose stack (fs-webrtc + FreeSWITCH + Caddy). This page covers the rest.

## Ports and firewall

| Port | Protocol | Who | Purpose |
|---|---|---|---|
| 80, 443 | TCP | Caddy | Web app, API, SIP WebSocket; HTTPS certificates |
| 16384-16584 | UDP | FreeSWITCH | Voice and video (about 4 ports per video call) |
| 8090 | TCP, 127.0.0.1 | fs-webrtc | Behind Caddy |
| 5066, 5090, 8021 | TCP, 127.0.0.1 | FreeSWITCH | SIP WebSocket, SIP, event socket |

```bash
sudo ufw allow 80,443/tcp && sudo ufw allow 16384:16584/udp
```

Cloud providers (AWS, GCP, Azure, Oracle, Hetzner …) have their own firewall/security group:
open the same ports there.

## NAT and media

Browsers send audio and video to FreeSWITCH, which relays it to the other side. FreeSWITCH must
announce an IP address that browsers can reach: `FSW_EXT_IP`.

* **Server with a public IP, or 1:1 NAT (most VPSs, AWS, GCP):** leave `FSW_EXT_IP` empty; the
  public address is discovered with STUN at startup.
* **Home server behind a router:** forward UDP 16384-16584 (and TCP 80/443) to the server and
  leave `FSW_EXT_IP` empty, or set it to your public IP.
* **LAN only (office, no internet access needed):** set `FSW_EXT_IP` to the server's LAN IP and
  `FSW_DOMAIN` to that IP. Caddy issues a self-signed certificate; accept it once per device.
* **Restrictive networks** (UDP blocked): browsers need a TURN relay. `FSW_ICE_SERVERS` takes
  STUN/TURN URLs, but TURN servers that require a username and password are not supported yet
  ([help wanted](https://github.com/101t/fs-webrtc/issues)).

**Symptom of a wrong `FSW_EXT_IP`:** the call connects (timer runs) but nobody hears or sees
anything, and FreeSWITCH hangs up after about a minute.

## Behind your own reverse proxy

Already running nginx or Traefik on ports 80/443? Remove the `caddy` service from
`docker-compose.yml` and forward everything, WebSockets included, to `127.0.0.1:8090`:

```nginx
server {
    listen 443 ssl;
    http2 on;
    server_name call.example.com;
    # ssl_certificate ...; ssl_certificate_key ...;

    location / {
        proxy_pass http://127.0.0.1:8090;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_read_timeout 1h;            # keep idle SIP WebSockets open
    }
    location /freeswitch/ { return 404; } # FreeSWITCH-only endpoint (also password-protected)
}
```

## Without Docker

1. Install FreeSWITCH 1.10+ (with mod_xml_curl, mod_sofia, mod_opus).
2. Use [`contrib/freeswitch/freeswitch.xml`](../contrib/freeswitch/freeswitch.xml) as its
   `freeswitch.xml`, and give the FreeSWITCH service the `FSW_FREESWITCH_SECRET` and `FSW_EXT_IP`
   environment variables (`systemctl edit freeswitch` → `[Service]` `EnvironmentFile=/etc/fs-webrtc.env`).
3. Download `fs-webrtc-<version>-x86_64-linux.tar.gz` from
   [Releases](https://github.com/101t/fs-webrtc/releases) and follow the comments in
   [`contrib/systemd/fs-webrtc.service`](../contrib/systemd/fs-webrtc.service).
4. Put a TLS reverse proxy in front (Caddy: `caddy reverse-proxy --from call.example.com --to 127.0.0.1:8090`).

### Adding fs-webrtc to an existing FreeSWITCH

The shipped `freeswitch.xml` replaces the whole configuration. To keep yours, copy three pieces
from it instead: the `xml_curl.conf` binding (directory + dialplan), the `webrtc` Sofia profile,
and `mod_xml_curl` in `modules.conf`. If your stock config still has the demo users
(`1000`-`1019`, `default_password`), remove them.

## Operations

```bash
docker compose logs -f                 # all logs
curl -s https://call.example.com/health
make fs-cli                            # FreeSWITCH console (sofia status, show channels …)
docker compose pull && docker compose up -d                  # upgrade (or re-run install.sh)
docker compose stop fs-webrtc && docker compose cp fs-webrtc:/var/lib/fs-webrtc ./backup && docker compose start fs-webrtc
```

User data (accounts, contacts) lives in the `fsw-data` volume, certificates in `caddy-data`.
