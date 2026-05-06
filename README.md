# lumina-rcs

A Tauri + Vue desktop client for Google Messages / RCS, built on
[mautrix-gmessages/libgm](https://github.com/mautrix/gmessages). Visual
language ported from rquickshare.

## Layout

```
bridge/   Go binary that wraps libgm, speaks NDJSON over stdio.
app/      Tauri 2 + Vue 3 frontend. Spawns and supervises the bridge.
```

## Phase 1 (current)

Pair via QR → list conversations. Auth persists across restarts.

## Run

```bash
# One-shot: build the bridge and start the Tauri app.
./scripts/dev
```

The script does `go build` for the bridge then `pnpm tauri dev` for the
app. The app spawns the bridge child process automatically. Bridge
logs appear in the same terminal (stderr); UI ↔ bridge JSON traffic is
on stdin/stdout.

First time only:

```bash
cd app && pnpm install
```

Manual equivalent:

```bash
(cd bridge && go build -o bin/lumina-bridge .)
(cd app && pnpm tauri dev)
```

Auth is persisted to your platform's app local data dir, e.g.
`~/.local/share/com.slush97.lumina/auth.json` on Linux. Override with
`LUMINA_DATA_DIR=/some/path`.

Override the bridge binary path with `LUMINA_BRIDGE_BIN=/path/to/binary`.

## Pairing

Lumina uses Google's account-based (GAIA) pairing because the QR flow is
being phased out of Google Messages. Because Google blocks login from
WebKitGTK, you copy the session cookies in from a real browser:

1. Click **Paste cookies from browser** in Lumina.
2. Sign into `messages.google.com/web` in Chrome/Firefox/Brave/etc.
3. Open DevTools → Storage → Cookies → `https://google.com`, copy at
   least the `SAPISID` cookie (and ideally `HSID`, `SSID`, `SID`,
   `APISID` too).
4. Paste into Lumina, click **Pair**.
5. Lumina shows one emoji; your phone shows three — tap the matching
   one.

Full walkthrough in [`docs/COOKIE_PAIRING.md`](docs/COOKIE_PAIRING.md).

Your phone must have Google Messages set as the default SMS app and stay
online (WiFi or cellular).

## Bridge protocol

Newline-delimited JSON over stdio.

```
→ {"id":"<uuid>","method":"status"}
← {"id":"<uuid>","result":{"paired":false,"connected":false}}

← {"event":"qr","data":{"url":"https://messages.google.com/web/..."}}
← {"event":"paired","data":{"phone_id":"..."}}
← {"event":"ready"}
← {"event":"conversation_updated","data":{...}}
```

Methods: `status`, `pair` (legacy QR), `pair_gaia`, `connect`, `unpair`,
`list_conversations`.

## Roadmap

- Phase 2: read message threads, render bubbles.
- Phase 3: compose + send.
- Phase 4: read receipts, typing, reactions.
- Phase 5: production sidecar bundling via Tauri `externalBin`.
- Phase 6 (UX polish): read cookies directly from
  Chrome/Brave/Chromium's `Cookies` SQLite DB so the paste step
  disappears. See [`docs/COOKIE_PAIRING.md`](docs/COOKIE_PAIRING.md).
