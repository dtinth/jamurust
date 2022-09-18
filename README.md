# Jamurust

> **Note** This project is no longer maintained. I have now rewritten this in Go because I can’t be bothered to deal with the borrow checker, lifetimes, and memory management right now. Check out the [gojam](https://github.com/dtinth/gojam) project.

Lightweight Jamulus client written in Rust.

I create this project mainly to learn Rust and solve some very specific problems. So you will see a lot of hardcoding and lots of bad code in here.

## jam-listener

If you’re on an unstable internet connection and want to listen to a Jamulus server, you can use this tool.
It features an extra large jitter buffer of 96 frames to make the listening experience more tolerant to network jitter.

### Usage

```
./jam-listener --server 127.0.0.1:22124
```

This will output the sound as a raw PCM stream (signed 16-bit little-endian stereo) to stdout.
Here are some examples of how to use it with ffmpeg:

```sh
# Plays the stream
./jam-listener --server 127.0.0.1:22124 | ffplay -f s16le -ar 48000 -ac 2 -i -

# Saves 10 seconds of the stream to an MP3 file
./jam-listener --server 127.0.0.1:22124 | ffmpeg -f s16le -ar 48000 -ac 2 -t 10 -i - output.mp3 -y
```

An example Node.js HTTP server that can stream an arbitrary Jamulus server as a live MP3 broadcast is provided as an example in `contrib/radio`.

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
