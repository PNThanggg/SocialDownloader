export type Site = 'tiktok' | 'instagram' | 'youtube' | 'facebook' | 'other';
export type Engine = 'yt_dlp' | 'gallery_dl';
export type OptionKind = 'video' | 'audio' | 'image' | 'original_sound';

export type OptionNote =
  | { kind: 'no_audio' }
  | { kind: 'source_is_video' }
  | { kind: 'source_audio'; ext: string }
  | { kind: 'limited_formats' };

export interface MediaOption {
  id: string;
  kind: OptionKind;
  label: string;
  ext: string;
  filesize: number | null;
  directUrl: string | null;
  note: OptionNote | null;
}

export interface MediaItem {
  index: number;
  thumbnail: string | null;
  isVideo: boolean;
  options: MediaOption[];
}

export interface MediaInfo {
  sourceUrl: string;
  site: Site;
  engine: Engine;
  title: string;
  author: string | null;
  durationSecs: number | null;
  thumbnail: string | null;
  items: MediaItem[];
  limitedFormats: boolean;
}

export type Tool = 'yt_dlp' | 'gallery_dl' | 'ffmpeg' | 'deno';

export type ErrorCode =
  | { code: 'unsupported_url' }
  | { code: 'login_required'; site: Site }
  | { code: 'private_or_removed' }
  | { code: 'extractor_broken' }
  | { code: 'network' }
  | { code: 'rate_limited' }
  | { code: 'youtube_bot_check' }
  | { code: 'playlist_not_supported' }
  | { code: 'tool_missing'; tool: Tool }
  | { code: 'already_running' }
  | { code: 'unknown' };

export interface LumiError {
  error: ErrorCode;
  detail: string;
}

export interface ToolStatus {
  tool: Tool;
  installed: boolean;
  version: string | null;
}

/** What "Update" did: every tool's status afterwards, plus the tools that could not be updated. */
export interface ToolUpdateReport {
  statuses: ToolStatus[];
  errors: { tool: Tool; error: LumiError }[];
}

export interface ToolProgress {
  tool: Tool;
  downloaded: number;
  total: number | null;
}

export interface AccountStatus {
  site: Site;
  connected: boolean;
}

export type JobState =
  | { state: 'started' }
  | { state: 'progress'; fraction: number | null }
  | { state: 'postprocessing' }
  | { state: 'done'; path: string }
  | { state: 'failed'; error: LumiError }
  | { state: 'cancelled' };

export type JobEvent = {
  jobId: string;
  url: string;
  optionId: string;
} & JobState;
