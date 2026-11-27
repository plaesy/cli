# plaesy agent

Serves the in-house agent-to-agent protocol over HTTP on loopback (`127.0.0.1`), backed by the project's relay database. Each sent message opens a task whose state other agents can read, stream, or cancel.

An older installed binary may lack this command. If `plaesy --help` does not list `agent`, use a build from source.

## Syntax

```sh
plaesy agent serve [--port 8765] [--path <REPO>]
```

| Option | Default | Meaning |
|---|---|---|
| `--port` | `8765` | Loopback port |
| `--path` | `.` | Repository whose `.plaesy/relay.sqlite` is served |

Set `PLAESY_AGENT_TOKEN` to require `Authorization: Bearer <token>` on every request. Without it, any local process can call the server.

## Routes

| Method and path | Effect |
|---|---|
| `GET /.well-known/agent.json` | Agent Card: name, version, capabilities, and the three skills below; never includes the token or paths |
| `POST /message:send` | Body `{"id","from","to","text"}`; opens a task and queues the message in one transaction. Repeating the same id and content returns the task's current state instead of an error |
| `GET /tasks/{from}/{id}` | Returns the task state as JSON |
| `GET /tasks/{from}/{id}/events` | Server-sent events stream of task state changes (closed after 10 minutes) |
| `POST /tasks/{from}/{id}:cancel` | Moves the task to `canceled` when its state allows it |

## Example

```sh
PLAESY_AGENT_TOKEN=<secret> plaesy agent serve --port 8765 &
curl -s -H "Authorization: Bearer <secret>" -X POST http://127.0.0.1:8765/message:send \
  -d '{"id":"m1","from":"builder","to":"reviewer","text":"review abc123"}'
curl -s -H "Authorization: Bearer <secret>" http://127.0.0.1:8765/tasks/builder/m1
```

## Safety

The server binds loopback only and blocks in the foreground; run it in a background terminal. The startup line on stderr says whether a token is required.

`plaesy agent serve --help` is authoritative.
