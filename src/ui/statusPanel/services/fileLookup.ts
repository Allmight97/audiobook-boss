import type { FileListInfo } from '../../../types/audio';

export function findFilePathByIndex(fileList: FileListInfo | null, index: number): string | null {
	if (!fileList) return null;
	if (!Number.isInteger(index)) return null;
	if (index < 0 || index >= fileList.files.length) return null;
	return fileList.files[index]?.path ?? null;
}
