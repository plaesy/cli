# plaesy remote

A read-only HTTP view of the project's task board. Nothing is written to `.plaesy/`.

## Syntax

```sh
plaesy remote serve [--port 7878] [--bind 127.0.0.1] [--auth-token <TOKEN>] [--root <PROJECT>]
```

| Option | Meaning |
|---|---|
| `--bind` | Default loopback. A non-loopback address requires a token |
| `--auth-token` | Bearer token of at least 32 bytes; default from `PLAESY_REMOTE_TOKEN`. When set, every route requires it |
| `--root` | Project root; default the nearest project above the current directory |

## Endpoints

| Route | Response |
|---|---|
| `GET /health` | Liveness and version |
| `GET /tasks` | JSON summary of tasks |
| `GET /events` | Server-sent events for task changes |

## Example

```sh
PLAESY_REMOTE_TOKEN=<32+ byte secret> plaesy remote serve --port 7878
curl -s -H "Authorization: Bearer <secret>" http://127.0.0.1:7878/tasks
```

## Safety

Binding beyond loopback exposes task titles on the network. Do it only with the user's approval and a strong token. The server runs in the foreground; start it in a background terminal.

`plaesy remote serve --help` is authoritative.
