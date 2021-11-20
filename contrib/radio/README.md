# jam-radio

A Node.js server that streams sound from a Jamulus server over the internet. Uses `jam-listener` and `ffmpeg`.

## Usage

```sh
export JAM_LISTENER=/path/to/jam-listener
node index.js
```

When started, the server will listen for incoming connections on port 8001.

A Docker container is also available, and here’s a `docker-compose.yml` file:

```yaml
version: '2'
services:
  jam-radio:
    image: 'ghcr.io/dtinth/jam-radio:main'
    restart: always
    ports:
      - 127.0.0.1:8001:8001
```

## API

### `GET /<ip>/<port>/listen.mp3`

Streams live audio from the Jamulus server.
It will connect to the target Jamulus server when the first listener connects, and disconnect when the last listener disconnects.
This means it will not work if the Jamulus server is already full.

**IMPORTANT:** The target Jamulus server must run on version `3.6.0` or higher, otherwise, noise will be generated (version check is not implemented, so just don’t do it, okay?).

## Security considerations

The Node.js server is intended to be used on a trusted network. It will happily connect to any Jamulus server specified in the URL.
Therefore you should not expose the server to the public internet without some kind of a reverse proxy that selectively allows a certain URLs or users.

### Setting up a reverse proxy

Using the [Caddy Web Server](https://caddyserver.com/) (a web server that requires very little configuration and provides automatic HTTPS), here’s an example of a Caddyfile that creates a public listening stream for your Jamulus server:

```
mydomain.tld {
    handle_path /listen/lobby.mp3 {
        reverse_proxy http://127.0.0.1:8001
        rewrite * /10.7.0.6/22124/listen.mp3
    }
}
```
