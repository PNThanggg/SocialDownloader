# Lumi

A small Windows desktop app (Tauri 2 + Svelte 5) that downloads media from TikTok, Instagram,
YouTube and Facebook: paste a link, pick MP4 / MP3 / image (or TikTok's original sound), save.

Lumi drives [yt-dlp](https://github.com/yt-dlp/yt-dlp) and [gallery-dl](https://github.com/mikf/gallery-dl),
which it downloads on first launch into `%LOCALAPPDATA%\Lumi\bin`. ffmpeg (MP3 and merging video with
sound) and deno (every YouTube quality) are downloaded the first time they are needed, after asking.
Instagram and Facebook can be connected in Settings for posts that need signing in; their cookies are
DPAPI-encrypted at rest.

The design lives in `docs/superpowers/specs/2026-10-25-lumi-design.md`.

## Develop

Requirements: Node 20+, Rust (MSVC toolchain), WebView2.

```sh
npm install
npm run tauri dev          # run the app
```

## Test

```sh
cd src-tauri && cargo test # Rust core (fake tools, no network)
npx vitest run             # UI
npm run check              # svelte-check + tsc
```

Network tests that use the real tools are ignored by default:
`cd src-tauri && cargo test -- --ignored --nocapture`.

## Build

```sh
npm run tauri build        # NSIS installer in src-tauri/target/release/bundle/nsis/
```

## CI / release (GitHub Actions)

- `.github/workflows/ci.yml`: every push to `master`/`main` and every pull request runs the type check,
  the UI tests and the Rust tests on Windows.
- `.github/workflows/release.yml`: pushing a tag such as `v0.1.0` (keep it equal to `version` in
  `src-tauri/tauri.conf.json`) builds the Windows installer and attaches it to a **draft** GitHub Release;
  publish the draft by hand. Running the workflow manually only builds, and the installer is kept as a run
  artifact.

```sh
git tag v0.1.0 && git push origin v0.1.0
```

macOS is not built yet: the app is Windows-only (DPAPI, Windows tool builds, Mica, per-site WebView2
profiles). The release matrix has a commented-out macOS entry to enable once the app is ported.
