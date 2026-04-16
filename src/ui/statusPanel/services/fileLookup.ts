import type { FileListInfo } from '../../../types/audio';

function fileBasename(filePath: string): string {
	return filePath.split(/[\\/]/).pop() || '';
}

export function findFilePathByName(fileList: FileListInfo | null, filename: string): string | null {
	if (!fileList) return null;
	const matches = fileList.files.filter((file) => fileBasename(file.path) === filename);
	return matches.length === 1 ? (matches[0]?.path ?? null) : null;
}

function stripProgressSuffix(value: string): string {
	const trimmed = value.trim();
	if (!trimmed) return '';
	const match = trimmed.match(/^(.*?) \(\d+\/\d+\)$/);
	return match?.[1]?.trim() ?? trimmed;
}

export function findFilePathByCurrentFile(
	fileList: FileListInfo | null,
	currentFile: string,
): string | null {
	if (!fileList) return null;

	const normalized = stripProgressSuffix(currentFile);
	if (!normalized) return null;

	const exactMatch = fileList.files.find((file) => file.path === normalized);
	if (exactMatch) {
		return exactMatch.path;
	}

	return findFilePathByName(fileList, fileBasename(normalized));
}

export function findFilePathByIndex(fileList: FileListInfo | null, index: number): string | null {
	if (!fileList) return null;
	if (!Number.isInteger(index)) return null;
	if (index < 0 || index >= fileList.files.length) return null;
	return fileList.files[index]?.path ?? null;
}
