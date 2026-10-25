# Lumi — Upgrade Spec: in-app browsing (mode B)

Date: 2026-10-25 · Status: prepared in advance, **not started**. Build it only when the user asks to move to mode B.

> **Language:** English — the source of truth for implementation. Vietnamese reading copy: `2026-10-25-lumi-browse-mode-design.vi.md`. If the two ever disagree, this file wins. Update both together.

Read first: `2026-10-25-lumi-design.md` (mode A). This spec assumes mode A already works and has its core separated as described in mode A §8.

Markers as in mode A: **[User]** = decided by the user, **[Claude]** = decided by the author, may be changed.

## 1. Purpose

The user browses Instagram and Facebook a lot. They said they're happy to "browse FB and IG right in the app" **[User]**. Mode B opens IG/FB inside Lumi: when they see a post they want to keep, they click one button instead of copying and pasting the link. Mode A (paste a link) stays as is **[Claude]**.

The user chose to build mode A first, with this spec ready so mode B can be implemented right away later **[User]**.

## 2. Goals / non-goals

**Goals**
- Open Instagram or Facebook in a Lumi window, already signed in with the account connected in mode A.
- On every post (or at least on the post currently open/being viewed), a floating **Download** button.
- Clicking it shows mode A's exact result card (MP3/MP4/images, Copy/Download) for that post.

**Non-goals** **[Claude]**
- Reading media files directly from the page (IG/FB video plays through `blob:`/MSE, so it's unusable). Always take the **post URL** and pass it through mode A's `resolve(url)`.
- TikTok/YouTube in the app (the user only mentioned FB and IG).
- Ad blocking or any IG/FB UI changes besides the Download button.

## 3. Changes to the main screen

- Add two small buttons next to the gear in layout A: **Browse Instagram** and **Browse Facebook** **[Claude]**. Layout A stays the main screen **[User]** chose.
- If the account isn't connected yet, clicking runs mode A's login flow first.

## 4. Browse window

- A new Tauri `WebviewWindow` loading `https://www.instagram.com/` or `https://www.facebook.com/`.
- Uses the **same WebView data directory** as mode A's login window (§7), so no re-login is needed **[Claude]**.
- A thin frosted-glass top bar with: Back / Forward / Reload, a read-only URL display, and a **Download this post** button (the fallback in §5.3).
  - *Implementation hint:* the bar is a separate Lumi UI webview. To combine it in one window, use Tauri's multi-webview feature (currently `unstable`). Simpler: put the toolbar in its own window attached above. The implementer chooses.
- Only allow navigation within `instagram.com`, `facebook.com`, `fbcdn.net`, `cdninstagram.com` and the related login/2FA domains. External links open in the default browser **[Claude]**.

## 5. Download button on posts

### 5.1 Injected script
- Use Tauri's `initialization_script` to inject a script into every IG/FB page.
- IG/FB are SPAs, so the script uses a `MutationObserver` to catch new posts on scroll and on in-app navigation.
- For each detected post, attach a small floating round button at the post's corner, inside its own Shadow DOM so page CSS can't override it.
- *[Claude]* Optionally add an `IntersectionObserver` to show the button only on the post occupying most of the screen, to reduce clutter.

### 5.2 Identifying the post and its URL
Every post must be reduced to a **canonical post URL** to pass to `resolve(url)`.

- **Instagram** (easy): each post is an `article` or a reel frame. Inside it is a link like `/p/<shortcode>/` or `/reel/<shortcode>/`, or a story `/stories/<user>/<id>/`. Canonical URL: `https://www.instagram.com/p/<shortcode>/`.
- **Facebook** (hard; obfuscated DOM that changes constantly): search the post frame for links matching `/reel/<id>`, `/videos/<id>`, `/watch/?v=<id>`, `/posts/<id>`, `/photo/?fbid=`, `permalink.php?story_fbid=`, `/share/`. Usually it's the link on the post's timestamp. If none is found, don't attach a button to that post.
- Keep selectors and link patterns in **a separate config file** (e.g. `browse-selectors.json`) so they can be fixed quickly when IG/FB change their DOM, without touching logic **[Claude]**.

### 5.3 Fallback: "Download this post"
When the user opens a post fully (the window URL is the post URL), the toolbar's **Download this post** button takes the current URL directly. This still works when the §5.1 script breaks because IG/FB changed their UI. **Build this first** so mode B is usable early, then add per-post buttons **[Claude]**.

### 5.4 Passing the URL from the page to Lumi (safely)
Do not expose Tauri IPC to external domains (i.e. no `remote` capability for instagram.com/facebook.com). That way IG/FB scripts can't call any Rust command **[Claude]**.

Proposed approach:
- On click, the injected script navigates to a special URL, e.g. `https://lumi.invalid/pick?url=<encoded>`.
- Rust intercepts this navigation with the window's `on_navigation` handler: cancel the navigation, read the `url` parameter, **validate** that it belongs to instagram.com/facebook.com and matches one of the §5.2 patterns, and only then call `resolve(url)`.
- As a result, even if the page itself navigates to this URL, it can only open the result popup. Downloading still requires a click in Lumi's UI and the Save as dialog.

## 6. Result popup

- Clicking the button opens a small always-on-top frosted-glass window next to the browse window. The popup shows **mode A's result card component** (mode A §4.2) with the freshly resolved `MediaInfo` **[Claude]**.
- Show a skeleton while resolving. Errors follow mode A's error table (§6).
- Downloads still go through the Windows Save as dialog **[User]** (decided in mode A).
- Esc or clicking outside closes the popup. Running downloads continue *[Claude]*.

## 7. Core reused from mode A (do not rewrite)

- `resolve(url) -> MediaInfo`
- `download(url, option_id, dest_path)`
- the `accounts` module (WebView data directory, DPAPI-encrypted cookie file)
- the result card component and the error table

Cookies: since the browse window shares the WebView data directory, cookies may get refreshed while browsing. **[Claude]** Each time the popup opens, re-read cookies from the WebView and overwrite the cookie file, so yt-dlp/gallery-dl always use the latest session.

## 8. Suggested implementation order

1. IG browse window + shared login + toolbar + **Download this post** (§5.3) + popup (§6).
2. Same for FB.
3. Per-post button script for IG (§5.1–5.2).
4. Per-post button script for FB.
5. Block unknown domains, open external links in the browser (§4).

After step 1 it's already usable; later steps make it more convenient.

## 9. Risks

- **IG/FB DOM changes often**, especially FB. Per-post buttons will break at times. Mitigation: the separate selector file (§5.2) and the fallback button (§5.3).
- **Account flagging:** browsing in an embedded WebView with an injected script may be detected by IG/FB as an unusual browser. Don't spoof the user agent excessively, don't auto-scroll or auto-click for the user. Every action is a user click.
- **Security:** no IPC for external domains (§5.4). Don't read or store page content beyond the chosen post URL.

## 10. Questions to ask the user when starting mode B

- Should the download button float on every post, or is the toolbar's "Download this post" button enough?
- Should the result popup be a separate small window or a side panel sliding out inside the browse window?
- Do they want TikTok added to browse mode?
