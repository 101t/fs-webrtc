# fs-webrtc developer & release tasks.  Run `make` to list them.
#
# Releasing:  update CHANGELOG.md (move "Unreleased" items under "## [x.y.z] - date"), then
#   make release-patch | release-minor | release-major     (or: make release VERSION=x.y.z)
#   git push origin HEAD vX.Y.Z
# The pushed tag triggers .github/workflows/release.yml: tests, Docker image on GHCR,
# static Linux binary (with the web app), and a GitHub Release with the CHANGELOG notes.

SHELL   := /bin/bash
IMAGE   ?= ghcr.io/101t/fs-webrtc
CURRENT := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
NEXT_PATCH := $(shell echo $(CURRENT) | awk -F. '{print $$1"."$$2"."$$3+1}')
NEXT_MINOR := $(shell echo $(CURRENT) | awk -F. '{print $$1"."$$2+1".0"}')
NEXT_MAJOR := $(shell echo $(CURRENT) | awk -F. '{print $$1+1".0.0"}')

.DEFAULT_GOAL := help
.PHONY: help build web release-build run web-dev test lint web-check fmt check docker docker-run up down logs \
        fs-cli version release release-patch release-minor release-major

help: ## List targets
	@awk 'BEGIN {FS = ":.*## "} /^[a-zA-Z_-]+:.*## / {printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2}' $(MAKEFILE_LIST)

# ---------------------------------------------------------------- develop

build: ## Debug build of the server
	cargo build

web: ## Build the web app -> web/dist
	cd web && npm ci --no-audit --no-fund && npm run build

release-build: web ## Optimized server + web app -> target/release/fs-webrtc, web/dist
	cargo build --release --locked

run: ## Run the server with settings from .env, serving web/dist (needs FreeSWITCH: make up)
	@set -a; [ -f .env ] && . ./.env; set +a; FSW_WEB_DIR=web/dist FSW_LOG_JSON=false cargo run

web-dev: ## Web app with hot reload on :5173, proxying to `make run`
	cd web && npm run dev

test: ## Server tests
	cargo test --locked

lint: ## Formatting and clippy, warnings are errors
	cargo fmt --check
	cargo clippy --all-targets --locked -- -D warnings

web-check: ## Type-check and build the web app
	cd web && npm ci --no-audit --no-fund && npm run build

fmt: ## Format the code
	cargo fmt

check: lint test web-check ## Everything CI runs

# ---------------------------------------------------------------- docker

docker: ## Build the image as $(IMAGE):dev
	docker build -t $(IMAGE):dev .

docker-run: docker ## Run the image on :8090 with settings from .env (FreeSWITCH not included)
	docker run --rm -it --env-file .env -p 8090:8090 $(IMAGE):dev

up: ## docker compose up (fs-webrtc + FreeSWITCH + Caddy), built from source
	docker compose up -d --build

down: ## docker compose down
	docker compose down

logs: ## Follow compose logs
	docker compose logs -f

fs-cli: ## FreeSWITCH console of the compose stack
	@set -a; . ./.env; set +a; docker compose exec freeswitch fs_cli -p "$$FSW_FREESWITCH_SECRET"

# ---------------------------------------------------------------- release

version: ## Print the current version
	@echo $(CURRENT)

release-patch: ## Release x.y.Z+1
	@$(MAKE) --no-print-directory release VERSION=$(NEXT_PATCH)

release-minor: ## Release x.Y+1.0
	@$(MAKE) --no-print-directory release VERSION=$(NEXT_MINOR)

release-major: ## Release X+1.0.0
	@$(MAKE) --no-print-directory release VERSION=$(NEXT_MAJOR)

release: ## Bump version, commit and tag vVERSION locally (make release VERSION=x.y.z)
	@[[ "$(VERSION)" =~ ^[0-9]+\.[0-9]+\.[0-9]+$$ ]] || { echo "usage: make release VERSION=x.y.z (current: $(CURRENT))"; exit 1; }
	@git diff --quiet && git diff --cached --quiet || { echo "commit or stash your changes first"; exit 1; }
	@! git rev-parse -q --verify "refs/tags/v$(VERSION)" >/dev/null || { echo "tag v$(VERSION) already exists"; exit 1; }
	@grep -q '^## \[$(VERSION)\]' CHANGELOG.md || { echo "CHANGELOG.md needs a '## [$(VERSION)] - $$(date +%F)' section"; exit 1; }
	awk '!done && /^version = / {print "version = \"$(VERSION)\""; done=1; next} {print}' Cargo.toml > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml
	cargo update --workspace --quiet
	$(MAKE) --no-print-directory check
	@# Nothing to commit when Cargo.toml already had VERSION (e.g. the very first release).
	git diff --quiet || git commit -am "chore(release): v$(VERSION)"
	git tag -a "v$(VERSION)" -m "fs-webrtc v$(VERSION)"
	@echo
	@echo "Tagged v$(VERSION). Publish it with:  git push origin HEAD v$(VERSION)"
