FROM rust:buster AS builder
RUN apt update && apt install llvm libclang-dev -y
COPY .git /opt/jamurust/.git/
COPY src /opt/jamurust/src/
COPY .gitignore .gitmodules build.rs Cargo.lock Cargo.toml wrapper.h /opt/jamurust/
WORKDIR /opt/jamurust/
RUN git submodule update --init
RUN cd opus && ./autogen.sh && ./configure --enable-static --disable-shared --enable-custom-modes --disable-hardening && make
RUN cargo build --target=x86_64-unknown-linux-gnu

FROM node:16-buster-slim
RUN apt-get update && apt-get install ffmpeg -y && rm -rf /var/lib/apt/lists/*
COPY contrib/radio/package.json contrib/radio/yarn.lock /opt/jam-radio/
WORKDIR /opt/jam-radio/
RUN yarn
COPY contrib/radio/ /opt/jam-radio/
COPY --from=builder /opt/jamurust/target/x86_64-unknown-linux-gnu/debug/jam-listener /usr/bin/jam-listener
ENV JAM_LISTENER=/usr/bin/jam-listener
CMD node .