import type { FileListInfo } from '../../types/audio';
import type { InputView } from '../inputSession';

export function fileListFromInput(view: InputView): FileListInfo | null {
	if (!view.hasFiles) {
		return null;
	}
	const files = [...view.files];
	const validCount = files.filter((file) => file.isValid).length;
	return {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: view.totalDurationSeconds,
		totalSize: files.reduce((sum, file) => sum + (file.size ?? 0), 0),
		validCount,
		invalidCount: files.length - validCount,
	};
}
