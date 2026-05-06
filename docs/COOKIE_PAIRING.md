# Cookie pairing

Lumina pairs with Google Messages by impersonating an authenticated web
client. To do that it needs the Google session cookies for your account.
Lumina can't acquire them itself because:

- **QR-code pairing is being removed** from Google Messages, so we can't
  fall back to the simple, cookie-free flow that the libgm library was
  originally written for.
- **Google blocks login from WebKitGTK** (the browser engine Tauri uses on
  Linux). The "Sign in with Google" window opens but never gets past
  the credentials screen — Google's anti-automation systems detect a
  non-Chrome / non-Firefox user agent and refuse to authenticate, even
  with a spoofed UA string.

So Lumina pulls the cookies out of a browser **you're already signed
into**. There are two ways:

1. **Automatic** — Lumina reads them from the browser's cookie store
   directly (decrypting via the OS keyring as needed). This is the
   default flow; click **Import from a browser** in Lumina. Skip to
   [Automatic import](#automatic-import) below.
2. **Manual** — copy the cookies out of DevTools and paste them in.
   Skip to [Manual paste](#manual-paste) below if the automatic flow
   fails or your browser isn't supported.

## Automatic import

Click **Import from a browser** on Lumina's pair screen. Lumina probes
for cookies in:

- **Chromium-family**: Chrome, Brave, Chromium, Edge, Vivaldi, Opera,
  Opera GX, Arc
- **Firefox-family**: Firefox, LibreWolf, Zen
- **macOS only**: Safari

It uses the [`rookie`](https://github.com/thewh1teagle/rookie) Rust
crate, which handles each browser's storage format and OS-specific
encryption (libsecret/kwallet on Linux, Keychain on macOS, DPAPI on
Windows).

### Caveats

- **Linux**: Lumina needs access to your login keyring (gnome-keyring,
  kwallet, or KeePassXC's secret-service module). On the first run you
  may see a prompt asking Lumina to read "Chrome Safe Storage" or
  similar — accept it.
- **Windows + Chrome ≥ 130**: Chrome started encrypting cookies with
  app-bound DPAPI in v130. Decrypting them requires Lumina to run with
  admin rights, or for you to use the manual paste flow.
- **Browser running**: most browsers lock the cookie SQLite while open;
  rookie copies the file before reading, so this usually works, but if
  it fails, close the target browser and retry.
- **Multiple profiles**: rookie reads the default profile. If you're
  signed into Google in a non-default profile, use manual paste.

## Manual paste

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

## Other options considered

The automatic-import path above is implemented via the `rookie` crate.
Two alternatives were rejected:

- **Browser extension.** Tiny WebExtension installed in your normal
  browser that POSTs cookies to a localhost endpoint Lumina runs.
  Polished UX but requires distributing and signing the extension.
- **Fingerprint-spoofing webview.** Patch WebKitGTK to look more
  convincingly like Chrome — feasible but Google updates its detection
  regularly, and we'd be playing cat-and-mouse forever.
