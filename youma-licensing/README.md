# Youma Licensing Server

Enterprise-grade software licensing server built in Rust. Supports seat, floating, trial, subscription, perpetual, and OEM licensing models. Ships with a full admin web UI, webhook delivery, audit trail, and background expiry/grace-period management.

---

## Quick Start

```bash
# 1. Clone and enter directory
git clone <repo-url> youma-licensing
cd youma-licensing

# 2. Copy environment config
cp .env.example .env
# Edit .env — set ADMIN_TOKEN and SIGNING_SECRET to 64-char random hex values.
# Generate them with: openssl rand -hex 32

# 3. Build and run
cargo build --release
./target/release/youma-licensing
# Admin UI: http://localhost:8080/admin
```

On first run, if `ADMIN_TOKEN` or `SIGNING_SECRET` are not set, the server generates random values and prints them to stderr. Copy them into your `.env` file.

---

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `DATABASE_PATH` | `./youma-licenses.db` | SQLite database file path |
| `ADMIN_TOKEN` | _(auto-generated)_ | Bearer token for admin API. SHA-256 hash stored in DB. |
| `SIGNING_SECRET` | _(auto-generated)_ | HMAC-SHA256 secret for validate/activate response signatures |
| `BIND_ADDR` | `0.0.0.0:8080` | Host and port to listen on |
| `TRIAL_DURATION_DAYS` | `30` | Default trial license duration |
| `MAX_RATE_PER_MIN` | `20` | Max public API requests per IP per minute |
| `LOG_LEVEL` | `info` | Log verbosity: `trace`, `debug`, `info`, `warn`, `error` |
| `LOG_FORMAT` | `pretty` | `pretty` (human) or `json` (structured, for log aggregators) |

---

## Licensing Models

| Model | Behavior |
|---|---|
| `seat` | 1 key = 1 machine. Pool has N seats. Machine-locked after first activation. |
| `floating` | Pool has N concurrent slots. App checks out on start, checks in on exit. Heartbeat required every 5 min. |
| `trial` | 30-day (configurable) auto-generated key. One trial per machine. |
| `subscription` | Has `expires_at`, renewable via admin API. Webhook fires 3 days before expiry. |
| `perpetual` | No expiry. Separate `maintenance_expires_at` (updates stop, app still works). |
| `oem` | Reseller gets a parent pool; distributes sub-pools to end customers. |

---

## API Reference

### Public Endpoints (rate-limited: 20 req/min/IP)

#### `POST /api/v1/license/activate`
Activate a license key on a machine.
```json
{ "key": "PROD-XXXX-XXXX-XXXX-XXXX", "machine_id": "raw-machine-guid", "email": "optional@example.com" }
```
Response:
```json
{ "ok": true, "plan": "pro", "features": ["meetings","ai","inbox"], "expires_at": "...", "maintenance_expires_at": null, "license_model": "seat", "signature": "hmac-hex" }
```

#### `POST /api/v1/license/validate`
Validate a license. Returns HMAC-signed response the client can verify offline.
```json
{ "key": "PROD-XXXX-...", "machine_id": "raw-machine-guid" }
```
Response includes `valid`, `reason` (null or `expired`/`suspended`/`machine_mismatch`/etc.), `grace_until`, and `signature`.

#### `POST /api/v1/license/deactivate`
Release a machine binding (increments transfer_count).
```json
{ "key": "PROD-XXXX-...", "machine_id": "raw-machine-guid" }
```

#### `POST /api/v1/license/floating/heartbeat`
Refresh a floating checkout (extend slot expiry by 5 minutes).
```json
{ "key": "PROD-XXXX-...", "machine_id": "raw-machine-guid" }
```

#### `POST /api/v1/license/trial`
Start a 30-day trial (one per machine).
```json
{ "machine_id": "raw-machine-guid", "email": "optional@example.com" }
```

#### `GET /api/v1/health`
Health check.
```json
{ "status": "ok", "version": "1.0.0", "db": "ok" }
```

### Admin Endpoints (Bearer token required)

All admin endpoints require `Authorization: Bearer <ADMIN_TOKEN>`.

#### Organizations
```
GET    /api/v1/admin/orgs              ?page=1&limit=20&q=search
POST   /api/v1/admin/orgs
GET    /api/v1/admin/orgs/:id
PUT    /api/v1/admin/orgs/:id
DELETE /api/v1/admin/orgs/:id
```

#### Pools
```
GET    /api/v1/admin/pools             ?org_id=...&model=...&plan=...
POST   /api/v1/admin/pools
GET    /api/v1/admin/pools/:id
POST   /api/v1/admin/pools/:id/generate   body: { "count": 50 }
GET    /api/v1/admin/pools/:id/keys    (paginated)
GET    /api/v1/admin/pools/:id/export  (CSV download)
```

#### Licenses
```
GET    /api/v1/admin/licenses          ?key=...&org=...&status=...&model=...
GET    /api/v1/admin/licenses/:key
POST   /api/v1/admin/licenses/:key/suspend        body: { "reason": "..." }
POST   /api/v1/admin/licenses/:key/unsuspend
POST   /api/v1/admin/licenses/:key/revoke         body: { "reason": "..." }
POST   /api/v1/admin/licenses/:key/extend         body: { "days": 90 }
POST   /api/v1/admin/licenses/:key/transfer
POST   /api/v1/admin/licenses/:key/set-machine-label  body: { "label": "..." }
PATCH  /api/v1/admin/licenses/:key/max-transfers  body: { "max": 5 }
```

#### Audit Log
```
GET    /api/v1/admin/audit             ?license_key=...&org_id=...&event=...&from=...&to=...
```

#### Dashboard
```
GET    /api/v1/admin/dashboard
```

#### Webhooks
```
GET    /api/v1/admin/webhooks
POST   /api/v1/admin/webhooks
PUT    /api/v1/admin/webhooks/:id
POST   /api/v1/admin/webhooks/:id/test
DELETE /api/v1/admin/webhooks/:id
```

---

## Webhook Events

Webhooks are delivered as `POST` with these headers:
```
X-Youma-Event: license.activated
X-Youma-Signature: sha256=<HMAC-SHA256(secret, body)>
X-Youma-Delivery: <uuid>
Content-Type: application/json
```

Available events: `license.activated`, `license.validated`, `license.deactivated`, `license.expired`, `license.suspended`, `license.revoked`, `license.extended`, `license.expiring_soon`, `trial.started`, `webhook.delivered`.

Failed deliveries are retried up to 5 times with exponential backoff (2^n minutes).

---

## Verify Signatures

The `signature` field in activate/validate responses is:
```
HMAC-SHA256(SIGNING_SECRET, "{valid}|{plan}|{expires_at}|{issued_at}")
```

The client should verify this before trusting the response. This allows offline validation when the server is temporarily unreachable.

---

## Deployment Guide

### systemd Service

Create `/etc/systemd/system/youma-licensing.service`:
```ini
[Unit]
Description=Youma Licensing Server
After=network.target

[Service]
Type=simple
User=youma
WorkingDirectory=/opt/youma-licensing
EnvironmentFile=/opt/youma-licensing/.env
ExecStart=/opt/youma-licensing/youma-licensing
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal
LimitNOFILE=65536

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now youma-licensing
sudo journalctl -u youma-licensing -f
```

### Nginx Reverse Proxy

```nginx
server {
    listen 80;
    server_name licensing.example.com;
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl http2;
    server_name licensing.example.com;

    ssl_certificate     /etc/letsencrypt/live/licensing.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/licensing.example.com/privkey.pem;

    location / {
        proxy_pass         http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header   Host $host;
        proxy_set_header   X-Real-IP $remote_addr;
        proxy_set_header   X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header   X-Forwarded-Proto $scheme;
        proxy_read_timeout 30s;
        client_max_body_size 2M;
    }
}
```

### Let's Encrypt TLS

```bash
sudo apt install certbot python3-certbot-nginx
sudo certbot --nginx -d licensing.example.com
```

---

## First Admin Setup

The server seeds a `superadmin` user on first startup using the `ADMIN_TOKEN` env var. Use this token in:
- The admin UI at `/admin` (enter token in the login dialog)
- Direct API calls: `Authorization: Bearer <ADMIN_TOKEN>`

To rotate the admin token:
1. Generate a new 64-char hex token: `openssl rand -hex 32`
2. Update `ADMIN_TOKEN` in `.env`
3. Restart the server (the DB row is updated on startup)

---

## Generating Your First Keys

1. Log into the admin UI at `/admin`
2. Create an **Organization** (e.g., "Acme Corp")
3. Create a **Pool** for that org (set model=`seat`, seats=100, plan=`pro`, duration=365)
4. Open the pool, click **Gen Keys**, enter a count, click Generate
5. Copy the keys or export as CSV
6. Distribute keys to customers

---

## Security Notes

- Machine IDs are **never stored raw** — only their SHA-256 hash.
- Admin token is stored as SHA-256 hash in the database.
- Response signatures use HMAC-SHA256 so clients can verify authenticity offline.
- Rate limiting: 20 req/min/IP on public endpoints (configurable).
- All timestamps are UTC ISO 8601.
- CORS is open (intended for desktop app clients).
- Run behind nginx with TLS in production.
