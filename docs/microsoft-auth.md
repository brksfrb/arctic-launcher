# Microsoft sign-in (for builders)

This is for people building Arctic Launcher themselves. Official releases already
include an approved client ID.

Microsoft sign-in needs an Azure application (client) ID. It is **not** stored in the
repository. Offline accounts work without any of this.

## 1. Register an Azure app

1. Sign in to the [Azure portal](https://portal.azure.com) → **Microsoft Entra ID** →
   **App registrations** → **New registration**.
2. **Name:** `Arctic Launcher` (or anything you like).
3. **Supported account types:** *Personal Microsoft accounts only*.
4. **Redirect URI:** platform **Public client/native (mobile & desktop)**, value
   `http://localhost`. Microsoft ignores the port for loopback redirects on public clients,
   so the launcher can use any free port.
5. Click **Register** and copy the **Application (client) ID**.
6. Under **Authentication → Advanced settings**, set **Allow public client flows** to **Yes**.
   The device-code flow needs this.
7. There is no client secret. The launcher is a public client using PKCE and device code.

## 2. Get the app approved for Minecraft

Since 2022, new Azure apps cannot call the Minecraft services API until Mojang allow-lists
them. Until then, login fails at the last step with *"Minecraft services rejected this app"*
(HTTP 403).

Request access through Mojang's app review form (<https://aka.ms/mce-reviewappid>) with your
client ID. Approval can take a while.

## 3. Give the client ID to the launcher

Pick one; the launcher checks them in this order:

1. **Environment variable** (good for development):
   ```powershell
   $env:ARCTIC_MSA_CLIENT_ID = "00000000-0000-0000-0000-000000000000"
   cargo run -p arctic-app
   ```
2. **Local config file** `%LOCALAPPDATA%\ArcticLauncher\msa.json`:
   ```json
   { "client_id": "00000000-0000-0000-0000-000000000000" }
   ```
3. **Baked in at build time**: if `ARCTIC_MSA_CLIENT_ID` is set while compiling, it becomes
   the default. The release workflow does this from a GitHub Actions secret of the same name.

A public client ID is not a secret (it ships inside every binary), but keeping it out of git
means forks register their own app and nothing is tied to the upstream registration.

## How the flows work

- **Sign in with browser**: opens `login.microsoftonline.com` with PKCE (S256) and a random
  `state`. The launcher listens on `127.0.0.1`/`::1` at an ephemeral port and exchanges the
  returned code.
- **Sign in with a code**: shows a short code to enter at `microsoft.com/link`, then polls
  the token endpoint until you finish.

Both then run Xbox Live → XSTS → Minecraft `login_with_xbox` → profile. Friendly errors cover
the common XSTS failures: no Xbox profile, child account, region.

Scopes: `XboxLive.signin offline_access`.

Tokens (the MSA refresh token and the Minecraft access token) are stored only in
`%LOCALAPPDATA%\ArcticLauncher\accounts.json`.
