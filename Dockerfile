FROM rust:latest

WORKDIR /app

VOLUME ["/data"]

ENV DATABASE_URL="sqlite:///data/database.db?mode=rwc"

RUN cargo install sqlx-cli

COPY . .

RUN sqlx migrate run

CMD ["cargo", "run", "--release"]