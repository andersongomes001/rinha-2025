FROM rust:1.87-bookworm AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    ca-certificates \
    && rm -rf /var/lib/apt/lists

WORKDIR /app
COPY . .
RUN rm -rf target
RUN RUSTFLAGS="-C target-cpu=skylake" cargo build --bin api --release
#RUN cargo build --bin api --release


FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    libgcc-s1 \
    libstdc++6 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/api /usr/local/bin/
RUN chmod +x /usr/local/bin/api
CMD ["api"]
