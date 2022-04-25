FROM rust:1.57.0

WORKDIR /usr/src/zfx-subzero
COPY . .

ENV CERTIFICATE_PATH="none"
ENV KEY_PATH="none"
ENV NODE_ID="none"
ENV NODE_ADDR="none"
ENV BOOTSTRAP_ID="none"
ENV BOOTSTRAP_ADDR="none"
ENV KEYPAIR="none"

RUN cargo build --release

ENTRYPOINT cargo run --package zfx-subzero --bin node -- -a $NODE_ADDR -b $BOOTSTRAP_ID@$BOOTSTRAP_ADDR --keypair $KEYPAIR --use-tls --cert-path $CERTIFICATE_PATH -p $KEY_PATH