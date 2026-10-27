FROM rust:1.83-bookworm AS build
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl libssl3 \
  && rm -rf /var/lib/apt/lists/*
COPY --from=build /app/target/release/admin-api /usr/local/bin/admin-api
ENV ADMIN_API_ADDR=0.0.0.0:8080
EXPOSE 8080
CMD ["admin-api"]
