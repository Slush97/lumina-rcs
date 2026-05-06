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

### Google Chrome / Chromium / Brave / Edge / Vivaldi

1. Sign into [`messages.google.com/web`](https://messages.google.com/web)
   with the Google account that owns your Google Messages.
2. Open DevTools (`F12` or `Ctrl+Shift+I`).
3. Switch to the **Application** tab.
4. In the left sidebar: **Storage → Cookies → `https://google.com`**
   (use `https://google.com`, not `https://messages.google.com` — the
   cookies you need live on the parent domain).
5. Find each row from the list above. For each cookie, double-click the
   `Value` column and copy the full string.
6. Paste into Lumina in either format:

   **JSON**

   ```json
   {
     "SAPISID": "abc123...",
     "HSID": "...",
     "SSID": "...",
     "SID": "...",
     "APISID": "..."
   }
   ```

   **DevTools "Copy" format** *(easier to assemble manually)*

   ```
   SAPISID=abc123...; HSID=...; SSID=...; SID=...; APISID=...
   ```

7. Click **Pair**. Lumina sends the cookies to the bridge, which
   contacts Google and returns a single emoji. Your phone's Google
   Messages app shows three emojis — tap the matching one to confirm.

### Firefox

1. Sign into `messages.google.com/web`.
2. Open DevTools (`F12`), go to the **Storage** tab.
3. **Cookies → `https://google.com`**.
4. Same as Chrome from step 5.

### Safari / mobile / others

Anywhere you can view cookies works. The cookie *names* and *values* are
all that matter — Lumina doesn't care about the source browser, expiry,
or other metadata.

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
