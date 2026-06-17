# ── Build stage ──────────────────────────────────────────────────
FROM rust:1.78-slim AS builder

WORKDIR /build

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo build --release --bin sorobangate 2>/dev/null || true

COPY . .
RUN touch src/main.rs src/lib.rs && \
    RUSTFLAGS="-C target-cpu=native" cargo build --release --bin sorobangate

# ── Runtime stage (distroless) ───────────────────────────────────
FROM gcr.io/distroless/cc-debian12:latest

COPY --from=builder /build/target/release/sorobangate /usr/local/bin/sorobangate

EXPOSE 8080 9000 9090

ENTRYPOINT ["/usr/local/bin/sorobangate"]
CMD ["--config", "/etc/sorobangate/config.toml"]
