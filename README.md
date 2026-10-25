# Lumi

A small Windows desktop app for saving media from TikTok, Instagram, YouTube and Facebook.
Paste a link, pick a format, choose where to save. No browser, no ads, no download sites.

Built with Tauri 2 (Rust) and Svelte 5.

## Features

- **Video, audio and images**: MP4 in each available quality, MP3 from any video, and single images or
  whole carousels (**Download all** saves every item into a folder).
- **TikTok original sound**: when a video uses a track, Lumi offers it as its own MP3 with title, artist
  and cover art, separate from the video's audio.
- **Download history**: every finished download is listed under the clock button with the post's
  original link. **Download again** reads that link afresh, so it works long after the direct media
  URLs have expired.
- **Instagram and Facebook sign-in**: connect an account in Settings for posts that need it. Each site
  signs in through its own in-app window, and its cookies are DPAPI-encrypted at rest.
- **Plain-language errors** with a way forward (connect, update the tools, retry), in Vietnamese and
  English.
- **Small installer** (about 2.5 MB) or a **portable** exe: the download tools are fetched on first
  launch, not bundled.

## How it works

Lumi drives [yt-dlp](https://github.com/yt-dlp/yt-dlp) and [gallery-dl](https://github.com/mikf/gallery-dl),
which it downloads on first launch. Two more tools are fetched only when first needed, after asking:
[ffmpeg](https://ffmpeg.org/) (MP3 and joining video with sound) and [deno](https://deno.com/) (every
YouTube quality). **Settings → Tools → Update** updates yt-dlp and gallery-dl.

Everything Lumi keeps lives in `%LOCALAPPDATA%\Lumi`:

| Path | What |
|---|---|
| `bin\` | yt-dlp, gallery-dl, ffmpeg, deno |
| `accounts\` | encrypted cookie jars for connected accounts |
| `webdata\accounts\<site>\` | each login window's own WebView profile |
| `history.json` | download history |
| `tmp\` | in-progress downloads, emptied on every start |

The design is in [`docs/superpowers/specs/2026-10-25-lumi-design.md`](docs/superpowers/specs/2026-10-25-lumi-design.md)
(Vietnamese copy alongside it).

## Requirements

Windows 10 or 11 (x64) with WebView2, which ships with Windows 11. macOS and Linux are not supported.

## Develop

You need Node 20+ and Rust with the MSVC toolchain.

```sh
npm install
npm run tauri dev
```

Dev-only screens with sample data, no network needed: `npm run dev`, then open
`http://localhost:5173/?demo=<name>`, for example `history`, `settings`, `instagram` or `error`
(the full list is in `src/lib/dev/demo.ts`).

## Test

```sh
npm run check                  # svelte-check + tsc
npx vitest run                 # UI
cd src-tauri && cargo test     # Rust core (fake tools, no network)
```

Tests that hit the network with the real tools are skipped by default:
`cd src-tauri && cargo test -- --ignored --nocapture`.

Before a release, walk through [`docs/manual-test-checklist.md`](docs/manual-test-checklist.md).

## Build

```sh
npm run tauri build
```

The installer is written to `src-tauri/target/release/bundle/nsis/Lumi_<version>_x64-setup.exe`.
It is not code-signed, so Windows SmartScreen warns on first install.

`src-tauri/target/release/Lumi.exe` is also the **portable** build: it runs without installing
(published as `Lumi_<version>_x64-portable.exe`). It still keeps its data in `%LOCALAPPDATA%\Lumi`, and it
needs the WebView2 runtime, which Windows 11 has and the installer fetches when missing.

## Note

Lumi is for personal use. Respect each platform's terms and the uploader's rights.
