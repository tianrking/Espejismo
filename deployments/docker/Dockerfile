FROM rust:1-bookworm AS builder
WORKDIR /src
COPY . .
RUN cargo build --release --workspace

FROM debian:bookworm-slim
RUN useradd --system --create-home --shell /usr/sbin/nologin espejismo \
    && mkdir -p /etc/espejismo \
    && chown -R espejismo:espejismo /etc/espejismo
COPY --from=builder /src/target/release/espejismo-local /usr/local/bin/espejismo-local
COPY --from=builder /src/target/release/espejismo-remote /usr/local/bin/espejismo-remote
USER espejismo
ENTRYPOINT ["/usr/local/bin/espejismo-remote"]
