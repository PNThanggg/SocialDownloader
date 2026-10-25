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
