import { pathBasename } from '../../../lib/path/basename';
import type { AudioFile } from '../../../types/audio';

function fileBasename(filePath: string): string {
	return pathBasename(filePath, { fallback: 'empty' });
}

function findFilePathByName(titles: ReadonlyArray<AudioFile>, filename: string): string | null {
	const matches = titles.filter((file) => fileBasename(file.path) === filename);
	return matches.length === 1 ? (matches[0]?.path ?? null) : null;
}

function stripProgressSuffix(value: string): string {
	const trimmed = value.trim();
	if (!trimmed) return '';
	const match = trimmed.match(/^(.*?) \(\d+\/\d+\)$/);
	return match?.[1]?.trim() ?? trimmed;
}

export function findFilePathByCurrentFile(
	titles: ReadonlyArray<AudioFile>,
	currentFile: string,
): string | null {
	const normalized = stripProgressSuffix(currentFile);
	if (!normalized) return null;

	const exactMatch = titles.find((file) => file.path === normalized);
	if (exactMatch) {
		return exactMatch.path;
	}

	return findFilePathByName(titles, fileBasename(normalized));
}

export function findFilePathByIndex(
	titles: ReadonlyArray<AudioFile>,
	index: number,
): string | null {
	if (!Number.isInteger(index)) return null;
	return titles[index]?.path ?? null;
}
