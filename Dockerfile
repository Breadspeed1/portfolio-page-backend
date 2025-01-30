FROM rust:latest

WORKDIR /app

VOLUME ["/data"]

COPY . .

CMD ["cargo", "run", "--release"]