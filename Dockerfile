# syntax=docker/dockerfile:1

# Cross-compilation helper — resolves the correct toolchain for $TARGETPLATFORM
FROM --platform=$BUILDPLATFORM tonistiigi/xx:1.7.0 AS xx

# Build the JS frontend on the native host platform (no QEMU overhead)
FROM --platform=$BUILDPLATFORM node:20-alpine AS frontend
WORKDIR /workspace
COPY js js
COPY public public
COPY package*.json ./
RUN npm install && npm run build

# Compile Rust on the native host platform, cross-compiling to $TARGETPLATFORM
FROM --platform=$BUILDPLATFORM docker.io/rust:1-alpine AS build

COPY --from=xx / /

ARG TARGETPLATFORM

WORKDIR /workspace

# Native build tools plus the clang/lld toolchain xx drives the cross build with
RUN apk add --no-cache build-base clang lld
RUN xx-apk add --no-cache musl-dev gcc

# Add the Rust target triple for the destination platform
RUN rustup target add "$(xx-cargo --print-target-triple)"

# AES-NI and SSE speed up the key derivation and the database crypto, but the
# flags only exist on x86, so they are picked per target and read by every
# cargo invocation below
RUN case "$(xx-cargo --print-target-triple)" in \
        x86_64-*) printf '%s' '-Ctarget-cpu=sandybridge -Ctarget-feature=+aes,+sse2,+sse4.1,+ssse3' > /rustflags ;; \
        *)        : > /rustflags ;; \
    esac

# Build dependencies in their own layer so source changes don't recompile them
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && echo 'fn main() {}' > src/main.rs \
    && RUSTFLAGS="$(cat /rustflags)" xx-cargo build --release \
    && rm -rf src

COPY src src
RUN touch src/main.rs && RUSTFLAGS="$(cat /rustflags)" xx-cargo build --bins --release

# Collect binary from the target-specific output directory
RUN xx-cargo --print-target-triple | xargs -I{} \
    cp target/{}/release/keepass4web-rs /keepass4web


FROM scratch

COPY --from=frontend /workspace/public /public
COPY --from=build /keepass4web /keepass4web
COPY config.yml /conf/

EXPOSE 8080

VOLUME /conf

USER 1000:1000

ENV RUST_BACKTRACE=1

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
    CMD ["/keepass4web", "--config", "/conf/config.yml", "--health-check"]

CMD ["/keepass4web", "--config", "/conf/config.yml"]
