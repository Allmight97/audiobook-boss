import { tauriClient } from '../../../lib/tauri/client';
import { convertBytesToDataUrl } from '../formatting';

export function shouldSkipCoverArtRead(lastCoverArtPath: string | null, filePath: string): boolean {
	return lastCoverArtPath === filePath;
}

export async function readCoverArtDataUrl(filePath: string): Promise<string | null> {
	const metadata = await tauriClient.readAudioMetadata(filePath);

	if (!metadata.cover_art || metadata.cover_art.length === 0) {
		return null;
	}

	return convertBytesToDataUrl(metadata.cover_art);
}
