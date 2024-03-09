FROM rust:1-slim-buster AS build

RUN mkdir /app
COPY . /app
WORKDIR /app
RUN cargo build --release --all

FROM debian:buster-slim

COPY --from=build /app/target/release/rinha-app /app/
COPY --from=build /app/target/release/rinha-espora-embedded /app/
COPY --from=build /app/target/release/rinha-espora-server /app/
COPY --from=build /app/target/release/rinha-load-balancer /app/
COPY --from=build /app/target/release/rinha-load-balancer-tcp /app/
