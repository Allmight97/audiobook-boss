import type { AudioFile, FileListInfo, JobType } from '../../types/audio';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import { getMetadataForFile, getMetadataIntentPatchForFile } from '../metadataState';

export function firstValidFilePath(fileList: FileListInfo | null): string | null {
	if (!fileList?.files.length) {
		return null;
	}
	const firstValid = fileList.files.find((file) => file.isValid);
	return firstValid?.path ?? null;
}

export function resolveCoverOwnerPaths(
	jobType: JobType,
	fileList: FileListInfo | null,
	selectedFiles: AudioFile[],
): string[] {
	if (jobType === 'merge') {
		const mergeKey = firstValidFilePath(fileList);
		return mergeKey ? [mergeKey] : [];
	}

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

export function effectiveCoverForFile(filePath: string): number[] | null {
	const intentPatch = getMetadataIntentPatchForFile(filePath);
	const intentCover = readCoverArtFromIntentPatch(intentPatch);
	if (intentCover !== undefined) {
		return intentCover;
	}

	const stored = getMetadataForFile(filePath)?.cover_art;
	if (stored && stored.length > 0) {
		return stored;
	}

	return null;
}

function readCoverArtFromIntentPatch(
	intentPatch: MetadataIntentPatch | undefined,
): number[] | null | undefined {
	if (!intentPatch?.cover_art) {
		return undefined;
	}
	if (intentPatch.cover_art.op === 'clear') {
		return null;
	}
	if (intentPatch.cover_art.op === 'set') {
		return intentPatch.cover_art.value.length > 0 ? intentPatch.cover_art.value : null;
	}
	return undefined;
}

export function resolveCoverDisplayPath(
	jobType: JobType,
	fileList: FileListInfo | null,
	selectedFiles: AudioFile[],
): string | null {
	if (jobType === 'merge') {
		return firstValidFilePath(fileList);
	}

	const validSelected = selectedFiles.filter((file) => file.isValid);
	if (validSelected.length === 1) {
		return validSelected[0]?.path ?? null;
	}
	if (validSelected.length > 1) {
		const covers = validSelected.map((file) => effectiveCoverForFile(file.path));
		const firstCover = covers[0] ?? null;
		const allSame = covers.every((cover) => coverBytesEqual(cover, firstCover));
		return allSame ? (validSelected[0]?.path ?? null) : null;
	}

	return firstValidFilePath(fileList);
}
