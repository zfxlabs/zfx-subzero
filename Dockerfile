FROM rust:1.57.0

WORKDIR /usr/src/zfx-subzero
COPY . .

ENV NODE_ID="none"
ENV NODE_ADDR="none"
ENV BOOTSTRAP_ID="none"
ENV BOOTSTRAP_ADDR="none"
ENV KEYPAIR="none"

RUN cargo build --release

ENTRYPOINT cargo run --package zfx-subzero --bin node -- -a $NODE_ADDR -b $BOOTSTRAP_ID@$BOOTSTRAP_ADDR --keypair $KEYPAIR --id $NODE_ID