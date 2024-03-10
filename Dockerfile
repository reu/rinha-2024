FROM rust:1-slim-buster AS build

RUN mkdir /app
COPY . /app
WORKDIR /app
ENV RUSTFLAGS "-C target-feature=+crt-static"
RUN cargo build \
  --release \
  --all \
  --target x86_64-unknown-linux-gnu

FROM scratch

COPY --from=build /app/target/x86_64-unknown-linux-gnu/release/rinha-app /app/
COPY --from=build /app/target/x86_64-unknown-linux-gnu/release/rinha-espora-embedded /app/
COPY --from=build /app/target/x86_64-unknown-linux-gnu/release/rinha-espora-server /app/
COPY --from=build /app/target/x86_64-unknown-linux-gnu/release/rinha-load-balancer /app/
COPY --from=build /app/target/x86_64-unknown-linux-gnu/release/rinha-load-balancer-tcp /app/
