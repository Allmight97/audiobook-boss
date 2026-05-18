import { describe, expect, it, vi } from 'vitest';
import { Effect, runAppEffect } from '../../../lib/effect/appEffect';
import type { AudioFile, FileListInfo } from '../../../types/audio';
import {
	importAnalysisWorkflowExecution,
	makeImportAnalysisWorkflowServicesLayer,
	type ImportAnalysisWorkflowServices,
} from '../importAnalysisWorkflow';

function audioFile(path: string): AudioFile {
	return {
		path,
		isValid: true,
		duration: 1,
		size: 100,
		format: 'm4b',
	} as AudioFile;
}

function fileListInfo(files: AudioFile[] = [audioFile('/books/new.m4b')]): FileListInfo {
	return {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: files.length,
		totalSize: files.length * 100,
		validCount: files.length,
		invalidCount: 0,
	} as FileListInfo;
}

function makeHarness(overrides: Partial<ImportAnalysisWorkflowServices> = {}) {
	const services: ImportAnalysisWorkflowServices = {
		isOrderLocked: vi.fn(() => false),
		openFiles: vi.fn(async () => ['/books/new.m4b']),
		analyzeAudioFiles: vi.fn(async () => fileListInfo()),
		persistPendingMetadataDraftsForCurrentSelection: vi.fn(async () => true),
		appendFileList: vi.fn(),
		pushStatusPanelTransientStatus: vi.fn(),
		setFileImportError: vi.fn(),
		clearFileImportError: vi.fn(),
		console: { error: vi.fn() },
		...overrides,
	};

	return {
		services,
		layer: makeImportAnalysisWorkflowServicesLayer(services),
	};
}

describe('ImportAnalysisWorkflow', () => {
	it('blocks import while file order is locked', async () => {
		const harness = makeHarness({ isOrderLocked: vi.fn(() => true) });

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'clickToSelect',
				existingFiles: [],
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.openFiles).not.toHaveBeenCalled();
		expect(harness.services.setFileImportError).toHaveBeenCalledWith(
			'Order locked while processing. Wait for completion to add files.',
		);
	});

	it('does nothing when file picker is cancelled', async () => {
		const harness = makeHarness({ openFiles: vi.fn(async () => null) });

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'clickToSelect',
				existingFiles: [],
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.analyzeAudioFiles).not.toHaveBeenCalled();
		expect(harness.services.appendFileList).not.toHaveBeenCalled();
	});

	it('reports unsupported drops before backend analysis', async () => {
		const harness = makeHarness();

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'dropFiles',
				paths: ['/tmp/notes.txt', '/tmp/image.png'],
				existingFiles: [],
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.analyzeAudioFiles).not.toHaveBeenCalled();
		expect(harness.services.setFileImportError).toHaveBeenCalledWith(
			'No supported audio files dropped. Please use mp3, m4a, m4b, aac, wav, flac files.',
		);
	});

	it('filters supported drop paths before analysis', async () => {
		const harness = makeHarness();

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'dropFiles',
				paths: ['/tmp/a.wav', '/tmp/b.txt', '/tmp/c.M4B'],
				existingFiles: [],
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.analyzeAudioFiles).toHaveBeenCalledWith(['/tmp/a.wav', '/tmp/c.M4B']);
	});

	it('reports analysis failure without appending files', async () => {
		const cause = new Error('analysis failed');
		const harness = makeHarness({
			analyzeAudioFiles: vi.fn(async () => {
				throw cause;
			}),
		});

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'dropFiles',
				paths: ['/tmp/a.m4b'],
				existingFiles: [],
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.appendFileList).not.toHaveBeenCalled();
		expect(harness.services.setFileImportError).toHaveBeenCalledWith(
			`Failed to analyze files: ${cause}`,
		);
	});

	it('does not append files when metadata staging fails', async () => {
		const harness = makeHarness({
			persistPendingMetadataDraftsForCurrentSelection: vi.fn(async () => false),
		});

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'clickToSelect',
				existingFiles: [],
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.appendFileList).not.toHaveBeenCalled();
		expect(harness.services.setFileImportError).toHaveBeenCalledWith(
			'Fix metadata validation errors before adding files.',
		);
	});

	it('appends analyzed files and clears import errors on success', async () => {
		const existingFiles = [audioFile('/books/existing.m4b')];
		const analyzed = fileListInfo([audioFile('/books/new.m4b')]);
		const harness = makeHarness({
			analyzeAudioFiles: vi.fn(async () => analyzed),
		});

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'clickToSelect',
				existingFiles,
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.appendFileList).toHaveBeenCalledWith(analyzed, { existingFiles });
		expect(harness.services.clearFileImportError).toHaveBeenCalledTimes(1);
	});

	it('reports duplicate-only imports without asking file list to emit duplicate status', async () => {
		const existingFiles = [audioFile('/books/existing.m4b')];
		const analyzed = fileListInfo([audioFile('/books/existing.m4b')]);
		const harness = makeHarness({
			analyzeAudioFiles: vi.fn(async () => analyzed),
		});

		await runAppEffect(
			importAnalysisWorkflowExecution({
				type: 'dropFiles',
				paths: ['/books/existing.m4b'],
				existingFiles,
			}).pipe(Effect.provide(harness.layer)),
		);

		expect(harness.services.appendFileList).toHaveBeenCalledWith(analyzed, {
			existingFiles,
			showDuplicateStatus: false,
		});
		expect(harness.services.pushStatusPanelTransientStatus).toHaveBeenCalledWith(
			'No new files added. All analyzed files were already in the list.',
			{ ttlMs: 2000 },
		);
		expect(harness.services.clearFileImportError).toHaveBeenCalledTimes(1);
	});
});
