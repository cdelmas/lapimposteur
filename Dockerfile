FROM rust:1.33.0-slim-stretch as deps

RUN \
  mkdir -p /source/bin/src/ && \
  mkdir -p /source/lib/src && \
  touch /source/bin/src/main.rs && \
  touch /source/lib/src/lib.rs && \
  echo "fn main() {}" > /source/bin/src/main.rs
COPY Cargo.toml Cargo.lock /source/
COPY bin/Cargo.toml /source/bin/
COPY lib/Cargo.toml /source/lib/
WORKDIR /source/
RUN cargo build --release && \
  rm -rf ./target/release/.fingerprint/lapimposteur*

FROM deps as build

# now, build the app
COPY . .
RUN cargo build --release


FROM bitnami/minideb:stretch as release
RUN groupadd lapinou && useradd -g lapinou lapinou
USER lapinou

COPY --from=build /source/target/release/lapimposteur lapimposteur

ENTRYPOINT ["/lapimposteur"]

