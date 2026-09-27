import type { AudioFile } from '../../types/audio';

export type FileListAppendResult =
	| { readonly outcome: 'duplicateOnly' }
	| {
			readonly outcome: 'replace' | 'append';
			readonly files: AudioFile[];
			readonly appendedFiles: AudioFile[];
	  };

function collectUniqueFiles(
	files: ReadonlyArray<AudioFile>,
	seenPaths: Iterable<string> = [],
): AudioFile[] {
	const seen = new Set(seenPaths);
	const uniqueFiles: AudioFile[] = [];
	for (const file of files) {
		if (seen.has(file.path)) {
			continue;
		}
		seen.add(file.path);
		uniqueFiles.push(file);
	}
	return uniqueFiles;
}

export function buildFileListAppendResult(
	incomingFiles: ReadonlyArray<AudioFile>,
	existingFiles: ReadonlyArray<AudioFile>,
): FileListAppendResult {
	const existing = collectUniqueFiles(existingFiles);
	if (existing.length === 0) {
		const files = collectUniqueFiles(incomingFiles);
		return { outcome: 'replace', files, appendedFiles: files };
	}

	const appendedFiles = collectUniqueFiles(
		incomingFiles,
		existing.map((file) => file.path),
	);
	if (appendedFiles.length === 0) {
		return { outcome: 'duplicateOnly' };
	}
	return { outcome: 'append', files: [...existing, ...appendedFiles], appendedFiles };
}
