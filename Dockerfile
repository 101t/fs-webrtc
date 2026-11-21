# syntax=docker/dockerfile:1.7
FROM node:26-alpine AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci --no-audit --no-fund
COPY web ./
RUN npm run build

FROM rust:1-slim-trixie AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY migrations migrations
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked && cp target/release/fs-webrtc /fs-webrtc

FROM debian:trixie-slim
RUN useradd --system --uid 10001 --home /var/lib/fs-webrtc fs-webrtc \
    && mkdir -p /var/lib/fs-webrtc && chown fs-webrtc /var/lib/fs-webrtc
COPY --from=build /fs-webrtc /usr/local/bin/fs-webrtc
COPY --from=web /web/dist /usr/share/fs-webrtc/web
USER fs-webrtc
ENV FSW_DATABASE_URL=sqlite:///var/lib/fs-webrtc/fs-webrtc.db \
    FSW_WEB_DIR=/usr/share/fs-webrtc/web \
    FSW_LOG_JSON=true
VOLUME /var/lib/fs-webrtc
EXPOSE 8090
HEALTHCHECK --interval=30s --timeout=3s CMD ["bash", "-c", "exec 3<>/dev/tcp/127.0.0.1/8090"]
ENTRYPOINT ["/usr/local/bin/fs-webrtc"]
