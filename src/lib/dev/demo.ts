// Dev-only fixtures for looking at the UI without network. Imported only under
// `import.meta.env.DEV` in App.svelte, so production builds drop this file.
import { language } from '../i18n'
import type { LumiError, MediaInfo } from '../types'

export interface Demo {
  url: string
  info?: MediaInfo
  error?: LumiError
  busy?: boolean
}

const MB = 1024 * 1024

const tiktok: MediaInfo = {
  sourceUrl: 'https://www.tiktok.com/@lumi.music/video/7381234567890123456',
  site: 'tiktok',
  engine: 'yt_dlp',
  title: 'Nhạc chill buổi tối 🎧 nghe là nghiện #lofi #chill',
  author: '@lumi.music',
  durationSecs: 48,
  thumbnail: null,
  limitedFormats: false,
  items: [
    {
      index: 0,
      thumbnail: null,
      isVideo: true,
      options: [
        { id: '0/video:1920', kind: 'video', label: '1080p', ext: 'mp4', filesize: 8.4 * MB, directUrl: 'https://v16.tiktokcdn.com/a.mp4', note: null },
        { id: '0/video:1280', kind: 'video', label: '720p', ext: 'mp4', filesize: 4.6 * MB, directUrl: 'https://v16.tiktokcdn.com/b.mp4', note: null },
        { id: '0/video:854', kind: 'video', label: '480p', ext: 'mp4', filesize: 2.2 * MB, directUrl: 'https://v16.tiktokcdn.com/c.mp4', note: null },
        { id: '0/audio', kind: 'audio', label: 'MP3', ext: 'mp3', filesize: 1.1 * MB, directUrl: 'https://v16.tiktokcdn.com/a.mp4', note: { kind: 'source_is_video' } },
        { id: 'sound', kind: 'original_sound', label: 'Tối nay em đi đâu — Lumi Lofi', ext: 'mp3', filesize: null, directUrl: 'https://sf16.tiktokcdn.com/s.mp3', note: null },
      ],
    },
  ],
}

const instagram: MediaInfo = {
  sourceUrl: 'https://www.instagram.com/p/C8xAbCdEfGh/',
  site: 'instagram',
  engine: 'gallery_dl',
  title: 'Đà Lạt mùa sương — weekend dump',
  author: '@lumi.travel',
  durationSecs: null,
  thumbnail: null,
  limitedFormats: false,
  items: [0, 1, 2, 3, 4].map((i) =>
    i === 3
      ? {
          index: i,
          thumbnail: null,
          isVideo: true,
          options: [
            { id: `${i}/video:1920`, kind: 'video', label: '1080p', ext: 'mp4', filesize: null, directUrl: 'https://scontent.cdninstagram.com/v.mp4', note: null },
            { id: `${i}/audio`, kind: 'audio', label: 'MP3', ext: 'mp3', filesize: null, directUrl: 'https://scontent.cdninstagram.com/v.mp4', note: { kind: 'source_is_video' } },
          ],
        }
      : {
          index: i,
          thumbnail: null,
          isVideo: false,
          options: [{ id: `${i}/image`, kind: 'image', label: 'JPG', ext: 'jpg', filesize: null, directUrl: `https://scontent.cdninstagram.com/${i}.jpg`, note: null }],
        },
  ),
}

const youtube: MediaInfo = {
  sourceUrl: 'https://www.youtube.com/watch?v=dQw4w9WgXcQ',
  site: 'youtube',
  engine: 'yt_dlp',
  title: 'Lo-fi hip hop radio — beats to relax/study to (1 hour mix)',
  author: 'Lumi Records',
  durationSecs: 3725,
  thumbnail: null,
  limitedFormats: true,
  items: [
    {
      index: 0,
      thumbnail: null,
      isVideo: true,
      options: [
        ...[1080, 720, 480, 360].map((h, i) => ({
          id: `0/video:${h}`,
          kind: 'video' as const,
          label: `${h}p`,
          ext: 'mp4',
          filesize: [612, 318, 166, 98][i] * MB,
          directUrl: 'https://rr1---sn.googlevideo.com/videoplayback',
          note: h > 360 ? ({ kind: 'no_audio' } as const) : null,
        })),
        { id: '0/audio', kind: 'audio', label: 'MP3', ext: 'mp3', filesize: 58.3 * MB, directUrl: 'https://rr1---sn.googlevideo.com/a', note: { kind: 'source_audio', ext: 'm4a' } },
      ],
    },
  ],
}

/** `name` or `name:vi` / `name:en` to also force the UI language. */
export function demo(spec: string): Demo {
  const [name, lang] = spec.split(':')
  if (lang === 'vi' || lang === 'en') language.set(lang)
  switch (name) {
    case 'instagram':
      return { url: instagram.sourceUrl, info: instagram }
    case 'youtube':
      return { url: youtube.sourceUrl, info: youtube }
    case 'loading':
      return { url: tiktok.sourceUrl, busy: true }
    case 'error':
      return {
        url: 'https://www.instagram.com/p/C8xPrivate/',
        error: { error: { code: 'login_required', site: 'instagram' }, detail: 'ERROR: [Instagram] login required' },
      }
    case 'unknown':
      return {
        url: 'https://www.facebook.com/share/r/abc/',
        error: { error: { code: 'unknown' }, detail: 'Traceback (most recent call last):\n  File "yt_dlp/__main__.py", line 17\nKeyError: \'video\'' },
      }
    default:
      return { url: tiktok.sourceUrl, info: tiktok }
  }
}
