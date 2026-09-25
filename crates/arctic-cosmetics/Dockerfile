# Build from the repository root:
#   docker build -f crates/arctic-cosmetics/Dockerfile -t arctic-cosmetics .
FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p arctic-cosmetics

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/arctic-cosmetics /usr/local/bin/arctic-cosmetics
COPY crates/arctic-cosmetics/assets /app/assets
WORKDIR /app
ENV ARCTIC_COSMETICS_ADDR=0.0.0.0:8080 \
    ARCTIC_COSMETICS_DB=/data/cosmetics.db \
    ARCTIC_COSMETICS_ASSETS=/app/assets
VOLUME /data
EXPOSE 8080
USER nobody
CMD ["arctic-cosmetics"]
