# foliolum-api as a small image. Built from this folder alone (its own workspace and Cargo.lock):
#
#   docker build -t foliolum-api services/foliolum-api
#
# The toolchain is the repo's (rust-toolchain.toml), and both stages are bookworm, so the binary's
# glibc is the one it runs on.
FROM rust:1.100.0-slim-bookworm@sha256:bb875b0f8ce726053e259034848c471840e96bfe591fd52556178ae9a05e6126 AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src src
COPY migrations migrations
COPY tests tests
RUN cargo build --release --locked --bin foliolum-api

FROM debian:bookworm-slim@sha256:e67eeead736ff7bbe258ef088bff683635c561b1fcfefad0806c1df4ab49ce0e
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates wget && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 --no-create-home foliolum
COPY --from=build /src/target/release/foliolum-api /usr/local/bin/foliolum-api
USER 10001
EXPOSE 8081
HEALTHCHECK --interval=30s --timeout=5s --retries=3 --start-period=10s CMD wget -q -O /dev/null http://127.0.0.1:8081/api/health || exit 1
ENTRYPOINT ["/usr/local/bin/foliolum-api"]
