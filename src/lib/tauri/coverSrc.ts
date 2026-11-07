import { convertFileSrc } from '@tauri-apps/api/core';

export function coverSrc(cover: string): string {
	return convertFileSrc(cover, 'abb-cover');
}
