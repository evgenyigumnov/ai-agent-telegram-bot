FROM rust:bullseye  as builder

RUN apt-get update && apt-get install -y build-essential  \
build-essential \
pkg-config \
libssl-dev \
            curl \
            perl \
    && rm -rf /var/lib/apt/lists/*


RUN cargo --version

WORKDIR /usr/src/app

COPY ./Cargo.toml .
COPY ./src ./src

RUN cargo build --release

FROM debian:bullseye


RUN mkdir /app
COPY --from=builder /usr/src/app/target/release/ai-agent-telegram-bot /app/bot
COPY .env /app/.env

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app


CMD ["./bot"]
