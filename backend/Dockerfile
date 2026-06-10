FROM rust:1.75 as builder
ARG BINARY=voyage_loader
WORKDIR /app
COPY . .
RUN cargo build --release -p ${BINARY}

FROM debian:bookworm-slim
ARG BINARY=voyage_loader
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/${BINARY} /usr/local/bin/app
COPY --from=builder /app/config.toml /app/config.toml
EXPOSE 3001 3002 3003
CMD ["app"]
