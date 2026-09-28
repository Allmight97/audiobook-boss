import type { AudioFile } from '../../types/audio';
import type { MetadataCache } from './cache';

function firstValidFilePath(files: ReadonlyArray<AudioFile>): string | null {
	return files.find((file) => file.isValid)?.path ?? null;
}

export function resolveCoverOwnerPaths(selectedFiles: AudioFile[]): string[] {
	const validSelected = selectedFiles.filter((file) => file.isValid);
	if (validSelected.length !== 1) {
		return [];
	}
	return [validSelected[0].path];
}

function coverBytesEqual(left: number[] | null, right: number[] | null): boolean {
	if (left === right) {
		return true;
	}
	if (!left || !right || left.length !== right.length) {
		return false;
	}
	for (let index = 0; index < left.length; index += 1) {
		if (left[index] !== right[index]) {
			return false;
		}
	}
	return true;
}

export function effectiveCoverForFile(filePath: string, cache: MetadataCache): number[] | null {
	const cover = cache.getMetadataForFile(filePath)?.cover_art;
	return cover && cover.length > 0 ? cover : null;
}

export function resolveCoverDisplayPath(
	files: ReadonlyArray<AudioFile>,
	selectedFiles: AudioFile[],
	cache: MetadataCache,
): string | null {
	const validSelected = selectedFiles.filter((file) => file.isValid);
	if (validSelected.length === 1) {
		return validSelected[0]?.path ?? null;
	}
	if (validSelected.length > 1) {
		const covers = validSelected.map((file) => effectiveCoverForFile(file.path, cache));
		const firstCover = covers[0] ?? null;
		const allSame = covers.every((cover) => coverBytesEqual(cover, firstCover));
		return allSame ? (validSelected[0]?.path ?? null) : null;
	}

	return firstValidFilePath(files);
}
