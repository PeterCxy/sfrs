# Build the application
FROM docker.io/debian:9-slim as builder

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN apt-get update && apt-get install -y sqlite3 libsqlite3-dev curl ca-certificates gcc

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /rustup-init.sh

RUN sh /rustup-init.sh  -y --no-modify-path --default-toolchain nightly

COPY . /sfrs

RUN cd /sfrs && cargo build --release && cp target/release/sfrs /usr/local/bin/

# Build the main image
FROM docker.io/debian:9-slim

RUN apt-get update && apt-get install -y sqlite3

COPY --from=builder /usr/local/bin/sfrs /usr/local/bin/sfrs

VOLUME ["/data"]
WORKDIR /data
EXPOSE 8000/tcp
ENTRYPOINT ["/usr/local/bin/sfrs"]
