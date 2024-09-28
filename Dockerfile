FROM rust:1-bookworm
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:12-slim
COPY --from=0 /app/target/release/knuckle_core /app/knuckle_core
ENV SEED_FILE=/app/data/server_seed
VOLUME /app/data
CMD ["/app/knuckle_core"]