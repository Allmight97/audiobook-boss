import type { AudioFile, FileListInfo } from '../../types/audio';

type SelectedDecoder = FileListInfo['selectedDecoders'][number];

export type FileListAppendOutcome = 'replace' | 'duplicateOnly' | 'append';

interface FileListAppendResultBase {
	readonly incomingFiles: AudioFile[];
	readonly appendedFiles: AudioFile[];
	readonly existingFiles: AudioFile[];
}

export type FileListAppendResult =
	| (FileListAppendResultBase & {
			readonly outcome: 'replace';
			readonly fileList: FileListInfo;
	  })
	| (FileListAppendResultBase & {
			readonly outcome: 'duplicateOnly';
			readonly fileList: null;
	  })
	| (FileListAppendResultBase & {
			readonly outcome: 'append';
			readonly fileList: FileListInfo;
	  });

export function collectUniqueFiles(
	files: AudioFile[],
	seenPaths: ReadonlySet<string> | null = null,
): AudioFile[] {
	const seen = new Set(seenPaths ?? []);
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

export function buildSelectedDecoderByPath(
	fileList: Pick<FileListInfo, 'files' | 'selectedDecoders'>,
): Map<string, SelectedDecoder> {
	const byPath = new Map<string, SelectedDecoder>();
	for (const [index, file] of fileList.files.entries()) {
		byPath.set(file.path, fileList.selectedDecoders[index] ?? null);
	}
	return byPath;
}

export function buildFileListInfoFromFiles(
	files: AudioFile[],
	decoderByPath: Map<string, SelectedDecoder> = new Map(),
): FileListInfo {
	const uniqueFiles = collectUniqueFiles(files);
	const selectedDecoders: SelectedDecoder[] = [];
	let totalDuration = 0;
	let totalSize = 0;
	let validCount = 0;
	let invalidCount = 0;

	for (const file of uniqueFiles) {
		selectedDecoders.push(decoderByPath.get(file.path) ?? null);
		if (file.isValid) {
			validCount += 1;
			totalDuration += file.duration ?? 0;
			totalSize += file.size ?? 0;
		} else {
			invalidCount += 1;
		}
	}

	return {
		files: uniqueFiles,
		selectedDecoders,
		totalDuration,
		totalSize,
		validCount,
		invalidCount,
	};
}

export function normalizeFileListInfo(fileListInfo: FileListInfo): FileListInfo {
	const decoderByPath = buildSelectedDecoderByPath(fileListInfo);
	return buildFileListInfoFromFiles(fileListInfo.files, decoderByPath);
}

export function buildFileListAppendResult(
	incomingFileList: FileListInfo,
	options: {
		readonly existingFiles: AudioFile[];
		readonly currentFileList: FileListInfo | null;
	},
): FileListAppendResult {
	const incomingFiles = collectUniqueFiles(incomingFileList.files);
	const existingFiles = collectUniqueFiles(options.existingFiles);

	if (existingFiles.length === 0) {
		return {
			outcome: 'replace',
			fileList: normalizeFileListInfo(incomingFileList),
			incomingFiles,
			appendedFiles: incomingFiles,
			existingFiles,
		};
	}

	const existingPathSet = new Set(existingFiles.map((file) => file.path));
	const appendedFiles = collectUniqueFiles(incomingFiles, existingPathSet);

	if (appendedFiles.length === 0) {
		return {
			outcome: 'duplicateOnly',
			fileList: null,
			incomingFiles,
			appendedFiles,
			existingFiles,
		};
	}

	const decoderByPath = new Map<string, SelectedDecoder>();
	if (options.currentFileList) {
		for (const [path, selection] of buildSelectedDecoderByPath(options.currentFileList)) {
			decoderByPath.set(path, selection);
		}
	}
	for (const [path, selection] of buildSelectedDecoderByPath(incomingFileList)) {
		decoderByPath.set(path, selection);
	}

	return {
		outcome: 'append',
		fileList: buildFileListInfoFromFiles([...existingFiles, ...appendedFiles], decoderByPath),
		incomingFiles,
		appendedFiles,
		existingFiles,
	};
}
