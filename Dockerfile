# Draughts64 boardimage — see SPEC.md
# Multi-stage: build in rust:alpine, ship a tiny distroless-style image.
FROM rust:1-alpine AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
# Pre-build the dep tree for layer caching (empty src placeholder trick).
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src
COPY src ./src
COPY tests ./tests 2>/dev/null || true
RUN touch src/main.rs && cargo build --release

FROM alpine:3.20
RUN addgroup -S boardimage && adduser -S boardimage -G boardimage
COPY --from=builder /build/target/release/boardimage /usr/local/bin/boardimage
USER boardimage
EXPOSE 8080
ENV BIND=0.0.0.0:8080
HEALTHCHECK --interval=15s --timeout=3s CMD wget -qO- http://127.0.0.1:8080/healthz || exit 1
CMD ["boardimage"]
