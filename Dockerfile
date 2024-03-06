FROM rust:1-slim-buster AS build

RUN cargo new --bin app
WORKDIR /app
RUN cargo new --lib espora-db
RUN cargo new --bin rinha-app
RUN cargo new --bin rinha-load-balancer
RUN cargo new --bin rinha-load-balancer-tcp

COPY Cargo.toml /app/
COPY Cargo.lock /app/
RUN cargo build --release

COPY espora-db /app/espora-db
COPY src /app/src
RUN touch /app/src/main.rs
RUN cargo build --release

FROM debian:buster-slim

COPY --from=build /app/target/release/rinha-2024 /app/rinha

CMD "/app/rinha"
