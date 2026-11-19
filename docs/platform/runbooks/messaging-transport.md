# Runbook: messaging transport, TLS, and key rotation

Scope: the shared inbox transport in `src/relay/transport.rs` (route `POST /v1/messages`). Owner of this runbook: the operator named in the support model (`docs/platform/decisions.md`, delegated decisions, item 7).

## 1. TLS termination

The transport speaks plain HTTP. It binds to loopback by default. It refuses a public bind unless TLS is terminated upstream. Terminate TLS at a reverse proxy on the same host.

Required:

- TLS 1.2 or later. Prefer TLS 1.3.
- A valid certificate for the public host name. Renew before expiry.
- The proxy forwards only `POST /v1/messages` to the loopback listener. Other paths return 404 at the proxy.
- A request body limit of 64 KiB at the proxy, matching the server limit.

Example (nginx, adjust names and paths):

```text
server {
    listen 443 ssl;
    server_name messages.example.internal;
    ssl_certificate     /etc/ssl/certs/messages.crt;
    ssl_certificate_key /etc/ssl/private/messages.key;
    ssl_protocols TLSv1.2 TLSv1.3;
    client_max_body_size 64k;

    location = /v1/messages {
        proxy_pass http://127.0.0.1:8787;
    }
    location / {
        return 404;
    }
}
```

Verify after each change:

- `openssl s_client -connect messages.example.internal:443 -tls1_1` must fail.
- A `GET /v1/messages` must return 404 or 405, never the inbox.

## 2. Key rotation

Keys are held in the OS credential store, one entry per key id, under the service name the deployment configures. Envelopes name their key id. Rotation runs in three steps, with an overlap window so no sender is cut off mid-flight.

1. Add the new key under a new id, for example `k2`. Do not delete `k1` yet. Senders still use `k1`.
2. Switch senders to `k2`. Wait at least the replay window (300 seconds) plus the longest expected send delay. Confirm in logs that `k1` no longer appears.
3. Retire `k1`: remove it from the credential store. Envelopes signed with `k1` now return 401 `unknown_key`.

Rules:

- Never reuse a key id. Ids are the audit trail.
- Never write keys to logs, envelopes, the repository, or chat.
- Rotate on any suspected exposure, and at least once a year.

## 3. Incidents

- 401 `bad_tag` from a known key: a sender is using the wrong key or the envelope was altered. Check the sender's key id first.
- 409 `replayed`: a sender retried with the same nonce. Treat as already received. Do not resend with the same nonce as a new message.
- 401 `expired`: the sender clock is more than 300 seconds off. Fix the clock, do not widen the window.
- Public bind refused at startup: correct the bind address, or confirm the proxy in section 1 before setting the upstream TLS flag.
