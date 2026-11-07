import { convertFileSrc } from '@tauri-apps/api/core';
import { COVER_URL_SCHEME } from '../generated/tauri';

export function coverSrc(cover: string): string {
	return convertFileSrc(cover, COVER_URL_SCHEME);
}
