# Jamurust

Lightweight Jamulus client written in Rust.

## jam-listener

If you’re on an unstable internet connection and want to listen to a Jamulus server, you can use this tool.
It is a lightweight Jamulus client with 96 frames of jitter buffer.

### Usage

```
./jam-listener --server 127.0.0.1:22124
```

This will output the sound as a raw PCM stream (signed 16-bit little-endian stereo) to stdout.
You can pipe it to `ffmpeg` to stream it, or `ffplay` to play it.

```
./jam-listener --server 127.0.0.1:22124 | ffplay -f s16le -ar 48000 -ac 2 -i -
```

## Building for Linux x64

```sh
# Run in Docker with an image with old enough libc,
# to avoid the "'glibc_2.29' not found" error when the binary
# is copied into a remote machine.
docker run -ti --rm -v $PWD:/workspace -w /workspace rust:buster

# Install dependencies for running bindgen
apt update
apt install llvm libclang-dev

# Fetch Opus
git submodule init
git submodule update

# Compile Opus and make it usable for static linking
bash -c 'cd opus && ./autogen.sh && ./configure --enable-static --disable-shared --enable-custom-modes --disable-hardening && make'

# Build the binary
cargo build --target=x86_64-unknown-linux-gnu
```
