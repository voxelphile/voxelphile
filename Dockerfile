FROM rust

WORKDIR /usr/src/app
COPY . .

RUN cargo build --release --bin backend

CMD ["./target/release/backend"]