# Lumi — Initial Design (mode A: paste a link)

Date: 2026-10-25 · Status: **approved** (user approved 2026-10-25)

> **Language:** English — the source of truth for implementation. Vietnamese reading copy: `2026-10-25-lumi-design.vi.md`. If the two ever disagree, this file wins. Update both together.

Markers: **[User]** = decided by the user. **[Claude]** = decided by the spec author, not confirmed by the user; the implementer may change it with good reason. Unmarked = general description.

## 1. Purpose and motivation

A personal Windows desktop tool to pull media from social networks onto the machine without hunting for third-party download sites. Two real situations that triggered it **[User]**:

- Saw a TikTok video with good music, wanted an **mp3** to listen to, but didn't want to go find the track elsewhere.
- Wanted to save **Instagram photos** to the machine.

Sources to support: TikTok, Instagram, YouTube, Facebook **[User]**. Other sites that yt-dlp/gallery-dl can read may work, but need no dedicated testing.

Success means: copy a link → paste into Lumi → a few seconds later the options appear → click Download → choose where to save → the file exists. No browser, no ads, no intermediate website.

## 2. Goals, non-goals, things to avoid

**Goals**
- Paste one link → show the **direct URL** (raw media URL) for each available format: MP3 (audio), MP4 (each quality), images **[User]**.
- Each row has a **Copy** button (copies the direct URL) and a **Download** button (saves the file) **[User]**.
- Download opens the **Windows "Save as" dialog** to choose the destination **[User]**.
- Connect Instagram and Facebook accounts through an **in-app login window** for posts that require sign-in **[User]**.
- Good-looking UI, small installer **[User]**.

**Out of scope for the first version** **[Claude]**, user has not objected
- Whole playlists/channels, multiple links at once, queues.
- Download history.
- macOS/Linux. Windows 10/11 x64 only.
- In-app Facebook/Instagram browsing (mode B). It has its own spec: `2026-10-25-lumi-browse-mode-design.md` **[User]**. Mode A must leave room to plug mode B in (see §8).
- Self-update of the Lumi app itself. Only the bundled tools update (see §5).

**Avoid, even if the app still "works"** **[Claude]**
- Heavy installer, slow startup.
- No visible sign that the app is working after a link is pasted.
- Errors shown as stack traces or raw yt-dlp logs. Every error must be translated into plain language with a way forward (see §6).
- Leaking the user's login cookies.

## 3. Approach (technology)

| Part | Choice | Decided by |
|---|---|---|
| App shell + UI | **Tauri 2** (Rust + WebView2) | [User] |
| Reading links, downloading video/audio | **yt-dlp** (standalone `yt-dlp.exe`) run as a child process | [Claude], user heard and agreed to this direction |
| MP3 conversion, video+audio merging | **ffmpeg** | [Claude] |
| Images (Instagram photo posts, TikTok photo slideshows, Facebook photos) | **gallery-dl** (standalone `.exe`) | [Claude]. Reason: yt-dlp only handles video; image-only Instagram posts usually fail. The user was not asked about this point. |
| Frontend | Svelte 5 + TypeScript + Vite | [Claude]. Small bundle. The implementer may switch frameworks if the bundle stays small. |
| UI language | **Vietnamese + English** | [User]. *[Claude]* Default follows the Windows display language (Vietnamese system → Vietnamese, anything else → English). Changeable in Settings. Every UI string and every error message (§6) exists in both languages. |

Python, Go and pure Rust were considered. Tauri was chosen because yt-dlp has no equivalent in Go/Rust, so yt-dlp has to run underneath regardless. Given that, a Tauri shell is the lightest way to get a polished UI.

## 4. Screens and behavior

The user picked from mockups: **layout A, a single compact screen** and **frosted-glass style** **[User]**.

### 4.1 Main screen
- Centered heading "Paste a link here" with the subline "TikTok · Instagram · YouTube · Facebook".
- A link input and a **Get links** button. Enter also triggers it.
- *[Claude]* On app start or when the window regains focus, if the clipboard holds a link from a supported site, show a small suggestion "Paste the link you just copied?". Never run without the user's consent.
- While working: the button switches to a loading state and the result area shows a skeleton. The screen must never sit frozen.
- A gear button in the top-right corner opens **Settings** (§4.5).

### 4.2 Result card
- Header: thumbnail, title, author, duration (if any).
- Below it, a list of rows. Each row: format tag (MP3 / MP4 / JPG…), info (quality, estimated size if known), **Copy** button, **Download** button.
- **Video:** list the qualities that actually exist, *[Claude]* grouped into friendly levels (1080p, 720p, 480p…), not the full yt-dlp format dump.
- **MP3:** always one MP3 row if the source has audio. *[Claude]* Best quality by default (`--audio-quality 0`).
- **TikTok original sound** **[User]**: TikTok videos often use an original track (a "sound"). That track can differ from the video's audio, since videos often have voice-over or use only a clip. When TikTok exposes the original sound, show **an extra, separate "Original sound" row** (track title + artist) next to the video's MP3 row. This row also has Copy/Download and downloads as MP3. If the original sound can't be obtained, don't show the row and don't show an error.
  - yt-dlp does not expose the original sound as a format. The implementer picks how to get it: read TikTok's post data (e.g. the `music.playUrl` field in page/API data), use gallery-dl, or something else. Verify early — this is the main thing the user needs.
  - *[Claude]* Write ID3 tags (title, artist, cover art if available) to the original-sound MP3. The video's MP3 row gets the video title and author.
- **Multi-image posts (carousel):** *[Claude]* show a thumbnail grid; each image has its own Copy/Download, plus a **Download all** button. "Download all" opens a **folder** picker instead of a file picker.
- **Mixed image + video posts:** each element is a grid cell; video cells show a play icon and offer both MP4 and MP3.

### 4.3 Copying the direct URL
- Copy copies the direct URL returned by yt-dlp/gallery-dl. The user only wants this link **to see it** **[User]**. No guarantee it opens outside the app, no proxy or intermediate link needed.
- *[Claude]* The UI (tooltip or small note) must state that direct links **may expire within hours and may not open in a browser** (TikTok/Instagram CDNs often require headers/cookies). The in-app Download button is unaffected.
- For MP3: there is no "direct MP3 URL" because the MP3 is created locally. *[Claude]* Copy on the MP3 row copies the source audio stream URL (usually m4a/webm). If the source only has a combined video+audio file (common on TikTok), copy that video URL. The tooltip says which source file is being copied.
- For high-quality YouTube video (separate video and audio streams): *[Claude]* Copy can only copy the video stream, and must note "no audio, use Download for the full file". If a combined format exists at that quality, prefer copying it.

### 4.4 Download
- Click **Download** → Windows Save as dialog opens **[User]**.
  - *[Claude]* File name pre-filled from the title (invalid characters removed). Default folder is the one the user picked last time; first time is Downloads.
- While downloading, the Download button becomes a progress bar (%) with a cancel button. When done, show an **Open folder** button. *[Claude]*
- *[Claude]* At most one download per row at a time. Other rows stay clickable (no real queue needed).
- Downloads go through yt-dlp/gallery-dl themselves (with cookies if connected). Never download over plain HTTP from the direct URL, so headers/cookies stay correct and video+audio merging is handled.
- *[Claude]* The Rust side owns downloads and their progress (each download has a job id) and broadcasts progress events to every window. A download is not tied to the window that started it. Mode B needs this, because downloads must keep running after the popup closes.

### 4.5 Settings (behind the gear button)
- **Accounts:** two rows, "Instagram" and "Facebook". Each shows its state (connected / not connected), a **Connect** button and a **Sign out** button. **[User]** chose in-app login.
- **Tools:** versions of yt-dlp, gallery-dl, ffmpeg, with an **Update** button.
- **Language:** Tiếng Việt / English **[User]**.
- *[Claude]* No default-quality setting in the first version.

### 4.6 Visual style — **[User]** chose "frosted glass"
- The window uses the **Mica** effect (Windows 11) via Tauri window effects, with a transparent window. *[Claude]* On Windows 10 (no Mica), fall back to Acrylic; if that also fails, use a dark tinted gradient background like the mockup.
- Blocks (input, result card, format rows) use a translucent white background around 6–12% + a faint white border + `backdrop-filter: blur`.
- The primary button is near-solid white with dark text. Format tags have their own colors: MP3 green, MP4 blue, image orange.
- *[Claude]* Custom (frameless) title bar with minimize/close buttons, draggable.
- Reference mockup: `.superpowers/brainstorm/*/content/style.html`, the "glass" option. That folder is not committed to git. The description above is all the implementer needs.

## 5. Bundled tools: download and update

Goal: small installer **[User]**. Therefore **[Claude]**:
- The installer does **not** include yt-dlp, gallery-dl or ffmpeg.
- **First launch:** download `yt-dlp.exe` and `gallery-dl.exe` from their official GitHub Releases into `%LOCALAPPDATA%\Lumi\bin\`, with a "Getting ready…" screen and progress.
- **ffmpeg** (about 80MB) is downloaded only **when first needed** (first MP3 download or first video that needs merging). Show "Lumi needs to download a converter (~80MB) once". Suggested source: the Windows build from `yt-dlp/FFmpeg-Builds` or gyan.dev (essentials build).
- **YouTube needs a JavaScript runtime:** current yt-dlp needs an external JS runtime (e.g. Deno) to get the full set of YouTube formats. *[Claude]* This runtime is also downloaded **when first needed** (first YouTube link), like ffmpeg, with a size notice. If the runtime is missing or its download fails, still run with fewer formats and the note "Not all qualities available".
- Verify checksums (SHA-256) when the source publishes them.
- **Update:** a button in Settings. *[Claude]* Also, when yt-dlp/gallery-dl report an extractor error, suggest "Try updating the tools" right in the error message. Never update silently in the background.
- No network on first launch: say so clearly and offer a retry button.

## 6. Errors must be in plain language

The Rust layer receives errors from yt-dlp/gallery-dl, classifies them, and returns an error code to the UI. The UI shows a message in the current language with an action button. Minimum set **[Claude]** (exact strings for every key live in the plan, Task 11):

| Situation | Vietnamese | English | Button |
|---|---|---|---|
| Link is not from a supported site / not a link | "Link này Lumi chưa đọc được." | "Lumi can't read this link yet." | — |
| Sign-in required (IG/FB) | "Bài này cần đăng nhập Instagram." | "This post requires signing in to Instagram." | **Connect Instagram** |
| Private / removed post | "Bài này riêng tư hoặc đã bị xóa." | "This post is private or was removed." | — |
| Extractor broken (site changed) | "Trang này vừa thay đổi, thử cập nhật công cụ." | "This site just changed, try updating the tools." | **Update** |
| No network | "Không có kết nối mạng." | "No network connection." | **Retry** |
| Temporarily blocked (rate limit) | "Trang đang chặn tạm, thử lại sau ít phút." | "The site is temporarily blocking requests, try again in a few minutes." | — |
| Anything else | "Có lỗi khi đọc link." | "Something went wrong reading this link." | **Show details** (opens the raw log) |

## 7. Account connection (in-app login)

**[User]** chose in-app login (the user uses IG/FB in Chrome, but recent Chrome versions block reading their cookies from outside, so Chrome cookies are not borrowed).

Flow:
1. Click **Connect Instagram** → open a new Tauri window (WebView2) loading `https://www.instagram.com/accounts/login/`.
2. The user signs in normally (including 2FA).
3. The app detects sign-in completion: the session cookie is present (`sessionid` for Instagram, `c_user` + `xs` for Facebook) or the URL has left the login page. It then reads that domain's cookies, **including HttpOnly**, via the Tauri/WebView2 cookie API (`Webview::cookies_for_url`, available since Tauri ~2.4, PR tauri-apps/tauri#12665), and closes the window.
   - *Risk to verify early:* whether this API returns all HttpOnly cookies for an external `https` domain on Windows. If not, the fallback is calling `ICoreWebView2CookieManager::GetCookies` directly via the `webview2-com` crate.
4. Write cookies to a Netscape-format `cookies.txt` (the format yt-dlp/gallery-dl read). **[Claude]** At rest, encrypt the file with Windows DPAPI (`CryptProtectData`, current-user scope). Decrypt to a temp file only while yt-dlp/gallery-dl run; delete the temp file afterwards.
5. When reading an IG/FB link: if connected, pass `--cookies <file>` to yt-dlp and `--cookies <file>` to gallery-dl.
6. **Sign out** = delete the cookie file + clear the login window's WebView data.

**[Claude]** The login window uses a **separate WebView data directory** (Lumi's own data directory), isolated from the main UI webview. Mode B will reuse exactly this directory to keep the session.

TikTok and YouTube need no sign-in in the first version **[Claude]**. Google blocks sign-in inside embedded WebViews, so in-app login can't work for YouTube. When YouTube demands "confirm you're not a bot", the app shows a clear error: "YouTube is asking to sign in, try again later or use another link". The first version has no workaround.

**Minimum per-source support for the first version to count as done** **[Claude]**:
- TikTok: video (MP4) + MP3 + the "Original sound" row when available. Photo slideshows if gallery-dl can read them.
- Instagram: images (including carousels), video/reels, MP3 from video.
- YouTube: public videos (MP4 at each quality) + MP3.
- Facebook: video/reels (MP4) + MP3. **Facebook photos are best-effort** via gallery-dl and do not block release if they don't work.

## 8. Structure for plugging in mode B later

Mode B (browse IG/FB in the app, download straight from a post) will reuse mode A's core. So mode A must separate its core as follows **[Claude]**:

- A core Rust command: `resolve(url) -> MediaInfo` (metadata + list of options, each with id, kind, label, direct URL). It knows nothing about the UI.
- A core command: `download(url, option_id, dest_path) -> progress stream`.
- An `accounts` module managing the login window, the WebView data directory and the cookie file.
- The result UI (the result card in §4.2) is a standalone component that takes a `MediaInfo`. Mode B will show this same component inside a popup.

Mode B details are in `2026-10-25-lumi-browse-mode-design.md`.

## 9. Packaging

- NSIS installer for Windows x64 (`tauri build`). **[Claude]**
- Installer size target: under **10MB** **[Claude]** (Tauri shell + frontend). WebView2 ships with Windows 11. On Windows 10 use the WebView2 bootstrapper.
- *[Claude]* No code signing in the first version, since it's a personal app. SmartScreen will warn on first install; that's acceptable.

## 10. Left to the implementer

- Project folder layout, module names, how to parse yt-dlp output (`-J` for metadata, `--newline --progress-template` for progress are suggestions).
- How to group formats into friendly quality levels.
- Motion, spacing and typography details, as long as they follow the frosted-glass spirit of §4.6.
- How to load thumbnails (thumbnail CDNs may require a referer; may need to fetch through Rust and pass back as a data URL).
- Testing: the Rust core (error classification, cookie → Netscape conversion, format grouping) should have unit tests. Real network calls can be tested manually with sample links.

## 11. Known risks

- Sites change their structure constantly. yt-dlp/gallery-dl will sometimes break until an update ships. That's why there's an Update button and a matching error message.
- Using real account cookies for automated downloads can get an Instagram/Facebook account flagged under heavy use. At personal usage levels the risk is low.
- Direct URLs expire and may not open outside the app (stated in the UI, §4.3).
- Downloading content must respect each platform's terms and the uploader's copyright. The app is for personal use.
