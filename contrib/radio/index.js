const express = require('express')
const execa = require('execa')
const uuid = require('uuid')
const dns = require('dns')
const app = express()

const remotes = {}

function createRemote(host, port) {
  const listeners = new Set()
  let active
  function go() {
    const exe = process.env.JAM_LISTENER || './target/debug/jam-listener'
    const child = execa(
      `${exe} --server ${host}:${port} | ffmpeg -f s16le -ar 48000 -ac 2 -i - -f mp3 -`,
      {
        stdio: ['ignore', 'pipe', 'inherit'],
        buffer: false,
        encoding: null,
        shell: true,
      },
    )
    child.on('error', () => {
      console.error('Child process error')
    })
    child.stdout.on('data', (data) => {
      listeners.forEach((listener) => {
        listener(data)
      })
    })
    return {
      dispose: () => {
        console.log('DISPOSE!!!!!!!!!!!!!!!!!!!!!!!!!!')
        child.stdout.destroy()
        child.kill()
      },
    }
  }
  let disposeTimeout = 0
  return {
    listen(callback) {
      listeners.add(callback)
      if (!active) {
        active = go()
      }
      clearTimeout(disposeTimeout)
      return () => {
        listeners.delete(callback)
        if (active && listeners.size === 0) {
          disposeTimeout = setTimeout(() => {
            if (active && listeners.size === 0) {
              active.dispose()
              active = null
            }
          }, 5000)
        }
      }
    },
  }
}

app.get('/:host/:port/listen.mp3', async (req, res, next) => {
  const requestId = uuid.v4()
  try {
    const { address } = await dns.promises.lookup(req.params.host, 4)
    const port = parseInt(req.params.port)
    if (!port) {
      res.status(400).send('Invalid port')
      return
    }
    const key = `${address}:${port}`
    const remote = remotes[key] || (remotes[key] = createRemote(address, port))
    const ip = req.ip
    const log = (m) => {
      console.log(
        `[${new Date().toJSON()}] [${ip} ${requestId}] ${m} => ${
          req.params.host
        }(${address}):${port}`,
      )
    }
    res.setHeader('Content-Type', 'audio/mp3')
    log('Response start')
    const unlisten = remote.listen((data) => {
      try {
        res.write(data)
      } catch (e) {
        log(`Cannot write: ${e.message}`)
      }
    })
    res.on('close', () => {
      unlisten()
      log('Response end')
    })
  } catch (e) {
    next(e)
  }
})

app.set('trust proxy', true)
app.use(express.static(__dirname + '/public'))

app.listen(8001, () => {
  console.log('Listening!')
})
