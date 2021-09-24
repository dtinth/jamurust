## Building

```sh
git submodule init
git submodule update
bash -c 'cd opus && ./autogen.sh && ./configure --enable-static --disable-shared --enable-custom-modes --disable-hardening && make'
cargo build
```
