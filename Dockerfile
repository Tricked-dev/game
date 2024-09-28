FROM debian:12-slim
COPY ./target/release/knuckle_core /app/knuckle_core
ENV SEED_FILE=/app/data/server_seed
VOLUME /app/data
CMD ["/app/knuckle_core"]