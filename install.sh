#!/bin/sh
# fs-webrtc installer: sets up fs-webrtc + FreeSWITCH + Caddy (HTTPS) with Docker Compose.
#
#   curl -fsSL https://raw.githubusercontent.com/101t/fs-webrtc/master/install.sh | sh
#
# Options (environment variables):
#   FSW_DOMAIN=call.example.com   domain or IP people open in the browser (asked if not set)
#   FSW_DIR=./fs-webrtc           where to install (default: ./fs-webrtc)
#   FSW_REF=v1.0.0                release to install (default: the latest release)
#
# Re-running it upgrades: files are refreshed, your .env is kept.
set -eu

REPO=101t/fs-webrtc
DIR=${FSW_DIR:-./fs-webrtc}

say() { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
die() { printf '\033[1;31merror:\033[0m %s\n' "$*" >&2; exit 1; }

command -v docker >/dev/null 2>&1 || die "Docker is required: https://docs.docker.com/engine/install/"
docker compose version >/dev/null 2>&1 || die "Docker Compose v2 is required (the 'docker compose' plugin)"
command -v curl >/dev/null 2>&1 || die "curl is required"

REF=${FSW_REF:-$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" 2>/dev/null |
  sed -n 's/.*"tag_name": *"\(v[0-9.]*\)".*/\1/p' | head -1)}
REF=${REF:-master}
case $REF in v[0-9]*) VERSION=${REF#v} ;; *) VERSION=latest ;; esac
say "Installing fs-webrtc $REF into $DIR"

mkdir -p "$DIR/contrib/freeswitch"
cd "$DIR"
for f in docker-compose.yml .env.example contrib/freeswitch/freeswitch.xml; do
  curl -fsSL "https://raw.githubusercontent.com/$REPO/$REF/$f" -o "$f" || die "download failed: $f"
done

if [ -f .env ]; then
  say "Keeping your existing .env"
  sed -i "s|^FSW_VERSION=.*|FSW_VERSION=$VERSION|" .env
else
  domain=${FSW_DOMAIN:-}
  if [ -z "$domain" ] && [ -r /dev/tty ]; then
    printf 'Domain name (or public IP) for this server, e.g. call.example.com: '
    read -r domain </dev/tty
  fi
  [ -n "$domain" ] || die "set FSW_DOMAIN, e.g. FSW_DOMAIN=call.example.com"
  case $domain in *[!A-Za-z0-9.:-]*) die "not a valid domain or IP: $domain" ;; esac
  secret=$(openssl rand -hex 32 2>/dev/null || od -An -N32 -tx1 /dev/urandom | tr -d ' \n')
  sed -e "s|^FSW_DOMAIN=.*|FSW_DOMAIN=$domain|" \
      -e "s|^FSW_FREESWITCH_SECRET=.*|FSW_FREESWITCH_SECRET=$secret|" \
      -e "s|^FSW_VERSION=.*|FSW_VERSION=$VERSION|" .env.example >.env
  chmod 600 .env
  say "Wrote .env (edit it to change settings, then: docker compose up -d)"
fi

say "Starting containers"
docker compose pull --quiet
docker compose up -d

domain=$(sed -n 's/^FSW_DOMAIN=//p' .env)
cat <<EOF

  fs-webrtc is starting at:  https://$domain

  Make sure this server accepts:
    TCP 80, 443          web app + HTTPS certificate      (e.g. ufw allow 80,443/tcp)
    UDP 16384-16584      voice and video                  (e.g. ufw allow 16384:16584/udp)
  and that the DNS A record of $domain points here.

  Create your account, then share the address with the people you want to call.
  Logs: cd $DIR && docker compose logs -f
EOF
