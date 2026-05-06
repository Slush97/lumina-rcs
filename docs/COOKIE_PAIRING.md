# Cookie pairing

Lumina pairs with Google Messages by impersonating an authenticated web
client. To do that it needs the Google session cookies for your account.
Lumina can't get them itself because:

- **QR-code pairing is being removed** from Google Messages, so we can't
  fall back to the simple, cookie-free flow that the libgm library was
  originally written for.
- **Google blocks login from WebKitGTK** (the browser engine Tauri uses on
  Linux). The "Sign in with Google" window opens but never gets past
  the credentials screen — Google's anti-automation systems detect a
  non-Chrome / non-Firefox user agent and refuse to authenticate, even
  with a spoofed UA string.

So for now: **you sign into Google in your real browser**, then
**copy the cookies into Lumina by hand**. This document explains how.

## What you need

Google Messages identifies a paired web client by a small set of cookies
on the `.google.com` domain. The minimum required is:

- `SAPISID`  *(required — Lumina rejects the paste if this is absent)*

The full set Lumina sends along (all helpful, none strictly required
beyond `SAPISID`):

- `SAPISID`
- `HSID`
- `SSID`
- `SID`
- `APISID`
- Any `__Secure-1PSID`, `__Secure-3PSID`, `__Secure-1PAPISID`,
  `__Secure-3PAPISID` if present

Paste any subset that includes `SAPISID`; Lumina will use what you give
it and report missing cookies.

## How to extract them

### The fast way (any modern browser, ~30 seconds)

Use the Network tab to copy the entire `Cookie` request header in one
shot. It contains every cookie your browser sends Google, in exactly
the format Lumina parses.

1. Sign into [`messages.google.com/web`](https://messages.google.com/web)
   with the Google account that owns your Google Messages.
2. Open DevTools (`F12` or `Ctrl+Shift+I`).
3. Switch to the **Network** tab.
4. **Reload the page** (so requests show up in the list).
5. Click any request to `messages.google.com` near the top of the list.
6. On the right, go to the **Headers** tab.
7. Scroll down to **Request Headers** and find the line that starts
   with **`cookie:`**.
8. Right-click the value → **Copy value** (Chrome/Brave/Edge) or
   **Copy Value** (Firefox).
9. Paste into Lumina's textarea, click **Pair**.

That single paste contains `SAPISID`, `HSID`, `SSID`, `SID`, `APISID`,
and every other cookie the browser is sending. Lumina forwards them all
to libgm.

### Manual — one cookie at a time

If you'd rather pick cookies individually (e.g. to keep less data on
disk), use the cookies panel directly:

1. Open DevTools → **Application** (Chrome) or **Storage** (Firefox).
2. **Cookies → `https://google.com`** (use `google.com`, not
   `messages.google.com` — the cookies live on the parent domain).
3. For each of `SAPISID`, `HSID`, `SSID`, `SID`, `APISID`: click the
   row, then in the bottom panel **Cookie Value**, click in the value
   field, `Ctrl+A`, `Ctrl+C`.
4. Assemble into either format Lumina accepts:

   **JSON**
   ```json
   { "SAPISID": "...", "HSID": "...", "SSID": "...", "SID": "...", "APISID": "..." }
   ```

   **Semicolon-delimited**
   ```
   SAPISID=...; HSID=...; SSID=...; SID=...; APISID=...
   ```

### Safari / mobile / others

Anywhere you can view a `Cookie` request header or the cookie store
works. The cookie *names* and *values* are all that matter.

## Security notes

These cookies are equivalent to your Google session token. Treat them as
passwords:

- Don't paste them into anything other than Lumina.
- Don't commit them to git (the `auth.json` file Lumina writes is in
  `.gitignore`).
- If you suspect they've leaked, sign out of all Google sessions at
  [Google Account → Security → Your devices](https://myaccount.google.com/security)
  to invalidate them immediately.

Lumina stores the cookies (along with the rest of the libgm `AuthData`
struct) at:

- Linux: `~/.local/share/com.slush97.lumina/auth.json` (mode `0600`)
- macOS: `~/Library/Application Support/com.slush97.lumina/auth.json`
- Windows: `%APPDATA%\com.slush97.lumina\auth.json`

The file is read once on startup to restore the session. After a
successful pair, you don't need to re-paste cookies unless the session
expires (typically two weeks of web inactivity per Google's policy) or
you explicitly **Unpair** in Lumina.

## Why not automate this?

Three options were considered:

1. **Read Chrome's cookie database directly.** Possible — Lumina could
   open `~/.config/google-chrome/Default/Cookies` (SQLite) and decrypt
   the `encrypted_value` column using Chrome's master key from
   `libsecret` / `kwallet`. Practical, ~half a day to implement
   correctly across Chrome / Brave / Chromium / Edge / Vivaldi (all use
   the same format). **Planned as a future enhancement** — see
   [`ROADMAP.md`](../README.md#roadmap).
2. **Browser extension.** Tiny WebExtension installed in your normal
   browser that POSTs cookies to a localhost endpoint Lumina runs.
   Polished UX but requires distributing and signing the extension.
3. **Fingerprint-spoofing webview.** Patch WebKitGTK to look more
   convincingly like Chrome — feasible but Google updates its detection
   regularly, and we'd be playing cat-and-mouse forever.

For now, the manual paste flow is the simplest thing that works
reliably and surfaces no surprises.
