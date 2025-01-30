FROM rust:latest

WORKDIR /app

VOLUME ["/data"]

RUN cargo install sqlx-cli

COPY . .

RUN sqlx migrate run

CMD ["cargo", "run", "--release"]