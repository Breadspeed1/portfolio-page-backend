FROM rust:latest

WORKDIR /app

VOLUME ["/data"]

ENV SQLX_OFFLINE=true

RUN cargo install sqlx-cli --no-default-features --features sqlite

COPY . .

COPY .sqlx .sqlx

RUN cargo build --release

CMD ["cargo", "run", "--release"]