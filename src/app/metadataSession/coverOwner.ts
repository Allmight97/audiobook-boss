import type { AudioFile, FileListInfo, JobType } from '../../types/audio';
import { getCoverDisplayForFile, getMetadataIntentPatchForFile } from './cache';
import type { CachedCoverDisplay } from './cache';

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

export type EffectiveCover = CachedCoverDisplay | { status: 'unknown' };

export function effectiveCoverForFile(filePath: string): EffectiveCover {
	const intentPatch = getMetadataIntentPatchForFile(filePath);
	const intentCover = intentPatch?.cover_art;
	if (intentCover?.op === 'clear') {
		return { status: 'cleared' };
	}
	const cached = getCoverDisplayForFile(filePath);
	if (intentCover?.op === 'set') {
		if (cached?.status === 'staged' && cached.handleId === intentCover.value) {
			return cached;
		}
		return {
			status: 'staged',
			handleId: intentCover.value,
			dataUrl: cached?.status === 'staged' ? cached.dataUrl : '',
		};
	}
	if (cached) {
		return cached;
	}
	return { status: 'unknown' };
}

function coversMatch(left: EffectiveCover, right: EffectiveCover): boolean {
	if (left.status !== right.status) {
		return false;
	}
	if (left.status === 'staged' && right.status === 'staged') {
		return left.handleId === right.handleId;
	}
	if (left.status === 'embedded' && right.status === 'embedded') {
		return left.dataUrl === right.dataUrl;
	}
	return true;
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
		const firstCover = covers[0] ?? { status: 'unknown' as const };
		const allSame = covers.every((cover) => coversMatch(cover, firstCover));
		return allSame ? (validSelected[0]?.path ?? null) : null;
	}

	return firstValidFilePath(fileList);
}
