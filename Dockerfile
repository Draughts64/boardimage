# Draughts64 boardimage — see SPEC.md
# Multi-stage: build in rust:alpine, ship a tiny distroless-style image.
FROM rust:1-alpine AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY tests ./tests
# NOTE: the classic empty-crate dep-cache trick breaks bin->lib (the
# placeholder lib gets cached without the real modules) — build plainly.
RUN cargo build --release && strip target/release/boardimage

FROM alpine:3.20
RUN addgroup -S boardimage && adduser -S boardimage -G boardimage
COPY --from=builder /build/target/release/boardimage /usr/local/bin/boardimage
USER boardimage
EXPOSE 8080
ENV BIND=0.0.0.0:8080
HEALTHCHECK --interval=15s --timeout=3s CMD wget -qO- http://127.0.0.1:8080/healthz || exit 1
CMD ["boardimage"]
