#Build Stage
FROM rust:1.71-slim AS builder

WORKDIR /app

ARG DATABASE_URL

ENV DATABASE_URL=$DATABASE_URL 

COPY . .

RUN cargo build --release

#production stage
FROM debian:buster-slim

WORKDIR /usr/local/bin

COPY --from=builder /app/target/release/rust_api .

CMD [ "./rust_api" ]
