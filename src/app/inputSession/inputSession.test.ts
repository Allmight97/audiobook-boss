import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AudioFile, FileListInfo, SupportedAudioImportMetadata } from '../../types/audio';
import { createAppRuntime, type AppRuntime } from '../runtime';
import type { InputCapability } from '../../lib/tauri/capabilities/input';

import { createInputOwner, chapterPlansForProcessing } from './index';
import { emptyInputSession, type InputSessionState } from './types';

const metadata: SupportedAudioImportMetadata = {
	formats: [{ extension: 'm4b', label: 'M4B' }],
	extensions: ['m4b'],
	formatsText: 'M4B',
	supportText: 'Supports M4B audio files',
};

function analyzedFile(path: string): FileListInfo {
	return {
		files: [
			{
				path,
				isValid: true,
				duration: 120,
				size: 1024,
				format: 'm4b',
				inputId: 'input-1',
			},
		],
		selectedDecoders: [null],
		totalDuration: 120,
		totalSize: 1024,
		validCount: 1,
		invalidCount: 0,
	};
}

function audioFile(path: string, overrides: Partial<AudioFile> = {}): AudioFile {
	return {
		path,
		inputId: path,
		isValid: true,
		duration: 60,
		size: 1024,
		format: 'm4b',
		...overrides,
	};
}

function sessionWith(files: AudioFile[], selected: number[] = []): InputSessionState {
	const fileList: FileListInfo = {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: files.length * 60,
		totalSize: files.length * 1024,
		validCount: files.length,
		invalidCount: 0,
	};
	const importOrdinalByPath: Record<string, number> = {};
	files.forEach((entry, index) => {
		importOrdinalByPath[entry.path] = index;
	});
	return {
		...emptyInputSession(),
		fileList,
		selectedIndices: selected,
		selectedAnchor: selected[selected.length - 1] ?? -1,
		importOrdinalByPath,
		nextImportOrdinal: files.length,
	};
}

function fakeInput(overrides: Partial<InputCapability> = {}): InputCapability {
	return {
		openFiles: vi.fn(async () => ['/books/chapter.m4b']),
		openDirectory: vi.fn(async () => '/books'),
		discoverAudioImportPaths: vi.fn(async (paths) => [...paths]),
		analyzeAudioFiles: vi.fn(async () => analyzedFile('/books/chapter.m4b')),
		getSupportedAudioImportMetadata: vi.fn(async () => metadata),
		takeOpenedAudioFiles: vi.fn(async () => []),
		readAudioCoverThumbnail: vi.fn(async () => null),
		listenDragDrop: vi.fn(async () => () => undefined),
		listenDragEnter: vi.fn(async () => () => undefined),
		listenDragLeave: vi.fn(async () => () => undefined),
		listenOpenedAudioFiles: vi.fn(async () => () => undefined),
		...overrides,
	};
}

describe('input session import tracer', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
	});

	it('imports a local file through the capability and exposes a renderer-ready row', async () => {
		const input = fakeInput();
		runtime = createAppRuntime({ input });
		await runtime.input.importIntent({ type: 'pickFiles' });
		const view = runtime.input.view();
		expect(view.files).toHaveLength(1);
		expect(view.files[0]?.path).toBe('/books/chapter.m4b');
		expect(view.selectedIndices).toEqual([0]);
		expect(view.errorMessage).toBe('');
		expect(input.analyzeAudioFiles).toHaveBeenCalledWith(['/books/chapter.m4b']);
	});

	it('records adapter failure as view error without mutating files', async () => {
		const input = fakeInput({
			analyzeAudioFiles: vi.fn(async () => {
				throw new Error('native boom');
			}),
		});
		runtime = createAppRuntime({ input });
		await runtime.input.importIntent({ type: 'importPaths', paths: ['/books/chapter.m4b'] });
		expect(runtime.input.view().errorMessage).toBe('Failed to analyze files. Please try again.');
		expect(runtime.input.view().files).toHaveLength(0);
	});
});

describe('input session selection gate', () => {
	it('blocks selectAll, clearSelection, and clearAllFiles when the gate returns false', async () => {
		const gate = vi.fn(async () => false);
		const owner = createInputOwner({ beforeSelectionChange: gate });
		owner.replaceSession(sessionWith([audioFile('/a'), audioFile('/b')], [0]));

		await owner.selectAll();
		expect(owner.session().selectedIndices).toEqual([0]);

		await owner.clearSelection();
		expect(owner.session().selectedIndices).toEqual([0]);

		await owner.clearAllFiles();
		expect(owner.session().fileList?.files).toHaveLength(2);
		expect(gate).toHaveBeenCalledTimes(3);
	});

	it('allows selectAll, clearSelection, and clearAllFiles when the gate returns true', async () => {
		const gate = vi.fn(async () => true);
		const owner = createInputOwner({ beforeSelectionChange: gate });
		owner.replaceSession(sessionWith([audioFile('/a'), audioFile('/b')], [0]));

		await owner.selectAll();
		expect(owner.session().selectedIndices).toEqual([0, 1]);

		await owner.clearSelection();
		expect(owner.session().selectedIndices).toEqual([]);

		await owner.clearAllFiles();
		expect(owner.session().fileList).toBeNull();
		expect(gate).toHaveBeenCalledTimes(3);
	});
});

describe('input session selection transition ticket', () => {
	it('ignores a stale gate answer when a newer selection completes first', async () => {
		let resolveFirstGate: (allowed: boolean) => void;
		const firstGate = new Promise<boolean>((resolve) => {
			resolveFirstGate = resolve;
		});
		let gateCall = 0;
		const gate = vi.fn(async () => {
			gateCall += 1;
			if (gateCall === 1) {
				return firstGate;
			}
			return true;
		});
		const owner = createInputOwner({ beforeSelectionChange: gate });
		owner.replaceSession(sessionWith([audioFile('/a'), audioFile('/b'), audioFile('/c')], [0]));

		const first = owner.selectFile({ index: 1, modifiers: { multi: false, range: false } });
		await owner.selectFile({ index: 2, modifiers: { multi: false, range: false } });
		resolveFirstGate!(true);
		const firstResult = await first;

		expect(firstResult).toBe(false);
		expect(owner.session().selectedIndices).toEqual([2]);
	});
});

describe('input session overlapping import', () => {
	it('does not let a stale drainOpened completion wipe a later lock error', async () => {
		let resolveOpened: ((paths: string[]) => void) | undefined;
		const opened = new Promise<string[]>((resolve) => {
			resolveOpened = resolve;
		});
		const input = fakeInput({
			takeOpenedAudioFiles: vi.fn(async () => opened),
		});
		const owner = createInputOwner({ capability: input });
		const drain = owner.importIntent({ type: 'drainOpened' });
		owner.setOrderLocked(true);
		const locked = owner.importIntent({ type: 'importPaths', paths: ['/tmp/file1.mp3'] });
		resolveOpened?.([]);
		await drain;
		await locked;
		expect(owner.view().errorMessage).toMatch(/Wait for completion to add files/);
		expect(owner.view().orderLocked).toBe(true);
		expect(input.analyzeAudioFiles).not.toHaveBeenCalled();
	});

	it('keeps an in-flight path import when a later drainOpened returns no files', async () => {
		let resolveAnalyze: ((list: FileListInfo) => void) | undefined;
		const analyzing = new Promise<FileListInfo>((resolve) => {
			resolveAnalyze = resolve;
		});
		const input = fakeInput({
			analyzeAudioFiles: vi.fn(async () => analyzing),
			takeOpenedAudioFiles: vi.fn(async () => []),
		});
		const owner = createInputOwner({ capability: input });
		const importing = owner.importIntent({
			type: 'importPaths',
			paths: ['/books/chapter.m4b'],
		});
		await vi.waitFor(() => expect(input.analyzeAudioFiles).toHaveBeenCalled());
		const drain = owner.importIntent({ type: 'drainOpened' });
		resolveAnalyze?.(analyzedFile('/books/chapter.m4b'));
		await importing;
		await drain;
		expect(owner.view().files).toHaveLength(1);
		expect(owner.view().files[0]?.path).toBe('/books/chapter.m4b');
		expect(owner.view().errorMessage).toBe('');
	});
});

describe('input session support text hydrate', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
	});

	it('does not restore a stale session when support text lookup fails after import', async () => {
		let rejectMetadata: (error: Error) => void;
		const metadataPromise = new Promise<SupportedAudioImportMetadata>((_resolve, reject) => {
			rejectMetadata = reject;
		});
		const input = fakeInput({
			getSupportedAudioImportMetadata: vi.fn(async () => metadataPromise),
		});
		runtime = createAppRuntime({ input });

		const hydratePromise = runtime.input.hydrateSupportText();
		await runtime.input.importIntent({ type: 'importPaths', paths: ['/books/chapter.m4b'] });
		rejectMetadata!(new Error('lookup failed'));
		await hydratePromise;

		expect(runtime.input.view().files).toHaveLength(1);
		expect(runtime.input.view().files[0]?.path).toBe('/books/chapter.m4b');
	});
});

it('carries folder CUE confirmation and explicit ignore into independent processing plans', async () => {
	const analyzed = analyzedFile('/books/book.mp3');
	analyzed.files[0] = {
		...analyzed.files[0]!,
		cueSource: {
			fileName: 'book.cue',
			status: 'needsConfirmation',
			message: 'Stale FILE name; using same-stem MP3.',
		},
		chapterPlan: {
			sourceFingerprint: '100:200',
			fromCue: true,
			chapters: [{ title: 'Opening', startMs: 940, endMs: 120000 }],
		},
	};
	analyzed.files.push({
		...audioFile('/books/broken.mp3'),
		inputId: 'broken',
		cueSource: {
			fileName: 'broken.cue',
			status: 'invalid',
			message: 'CUE line 2: multiple FILE sheets are unsupported',
		},
		chapterPlan: { sourceFingerprint: '200:300', fromCue: false, chapters: [] },
	});
	analyzed.validCount = 2;
	const capability = fakeInput({
		discoverAudioImportPaths: vi.fn(async () => ['/books/book.mp3', '/books/broken.mp3']),
		analyzeAudioFiles: vi.fn(async () => analyzed),
	});
	const owner = createInputOwner({ capability });
	await owner.importIntent({ type: 'pickFolder' });
	expect(capability.discoverAudioImportPaths).toHaveBeenCalledWith(['/books']);
	expect(owner.view().files[0]?.chapterPlan?.chapters).toHaveLength(1);
	expect(owner.view().files[0]?.cueSource?.fileName).toBe('book.cue');
	expect(() => chapterPlansForProcessing(owner.view().files, 'batch')).toThrow('confirm');
	owner.chooseCue('input-1', 'confirmHundredths');
	expect(() => chapterPlansForProcessing(owner.view().files, 'batch')).toThrow('broken.cue');
	expect(owner.view().files[1]?.cueSource?.message).toContain('multiple FILE');
	owner.chooseCue('broken', 'ignore');
	const accepted = chapterPlansForProcessing(owner.view().files, 'batch');
	expect(accepted?.['/books/book.mp3']?.chapters).toEqual([
		{ title: 'Opening', startMs: 940, endMs: 120000 },
	]);
	expect(accepted?.['/books/broken.mp3']?.chapters).toEqual([]);
	expect(() => chapterPlansForProcessing(owner.view().files, 'merge')).toThrow(
		'Merging CUE-bearing',
	);
	owner.chooseCue('input-1', 'ignore');
	expect(
		chapterPlansForProcessing(owner.view().files, 'merge')?.['/books/book.mp3']?.chapters,
	).toEqual([]);
	expect(accepted?.['/books/book.mp3']?.chapters[0]?.startMs).toBe(940);
});

it.each(['confirmHundredths', 'ignore'] as const)(
	'preserves CUE %s while another import is analyzed',
	async (choice) => {
		let finishAnalysis!: (files: FileListInfo) => void;
		const capability = fakeInput({
			analyzeAudioFiles: vi.fn(
				() =>
					new Promise<FileListInfo>((resolve) => {
						finishAnalysis = resolve;
					}),
			),
		});
		const owner = createInputOwner({ capability });
		owner.replaceSession(
			sessionWith([
				audioFile('/books/book.mp3', {
					cueSource: {
						fileName: 'book.cue',
						status: 'needsConfirmation',
						message: 'Confirm timestamps',
					},
					chapterPlan: {
						sourceFingerprint: '100:200',
						fromCue: true,
						chapters: [{ title: 'Opening', startMs: 0, endMs: 60000 }],
					},
				}),
			]),
		);
		const pending = owner.importIntent({ type: 'importPaths', paths: ['/books/new.m4b'] });
		await vi.waitFor(() => expect(capability.analyzeAudioFiles).toHaveBeenCalled());
		owner.chooseCue('/books/book.mp3', choice);
		finishAnalysis(analyzedFile('/books/new.m4b'));
		await pending;
		expect(owner.view().files.map((file) => file.path)).toEqual([
			'/books/book.mp3',
			'/books/new.m4b',
		]);
		expect(owner.view().files[0]?.cueSource?.status).toBe(
			choice === 'ignore' ? 'ignored' : 'ready',
		);
		expect(
			chapterPlansForProcessing(owner.view().files, 'batch')?.['/books/book.mp3']?.chapters,
		).toHaveLength(choice === 'ignore' ? 0 : 1);
	},
);

it.each(['remove', 'clear'] as const)(
	'does not restore rows after %s while another import is analyzed',
	async (action) => {
		let finishAnalysis!: (files: FileListInfo) => void;
		const capability = fakeInput({
			analyzeAudioFiles: vi.fn(
				() =>
					new Promise<FileListInfo>((resolve) => {
						finishAnalysis = resolve;
					}),
			),
		});
		const owner = createInputOwner({ capability });
		owner.replaceSession(sessionWith([audioFile('/books/old.m4b')]));
		const pending = owner.importIntent({ type: 'importPaths', paths: ['/books/new.m4b'] });
		await vi.waitFor(() => expect(capability.analyzeAudioFiles).toHaveBeenCalled());
		if (action === 'remove') owner.removeFile(0);
		else await owner.clearAllFiles();
		finishAnalysis(analyzedFile('/books/new.m4b'));
		await pending;
		expect(owner.view().files.map((file) => file.path)).toEqual(['/books/new.m4b']);
	},
);

it.each(['cancel', 'error'] as const)(
	'keeps row removal when an outstanding picker ends with %s',
	async (outcome) => {
		let finishPicker!: (paths: string[] | null) => void;
		let failPicker!: (error: Error) => void;
		const capability = fakeInput({
			openFiles: vi.fn(
				() =>
					new Promise<string[] | null>((resolve, reject) => {
						finishPicker = resolve;
						failPicker = reject;
					}),
			),
		});
		const owner = createInputOwner({ capability });
		owner.replaceSession(sessionWith([audioFile('/books/old.m4b')]));
		const pending = owner.importIntent({ type: 'pickFiles' });
		await vi.waitFor(() => expect(capability.openFiles).toHaveBeenCalled());
		owner.removeFile(0);
		if (outcome === 'cancel') finishPicker(null);
		else failPicker(new Error('picker failed'));
		await pending;
		expect(owner.view().files).toEqual([]);
		if (outcome === 'error') expect(owner.view().errorMessage).not.toBe('');
		else expect(owner.view().errorMessage).toBe('');
	},
);

it('rejects completed import analysis when processing locks the input meanwhile', async () => {
	let finishAnalysis!: (files: FileListInfo) => void;
	const capability = fakeInput({
		analyzeAudioFiles: vi.fn(
			() =>
				new Promise<FileListInfo>((resolve) => {
					finishAnalysis = resolve;
				}),
		),
	});
	const owner = createInputOwner({ capability });
	owner.replaceSession(sessionWith([audioFile('/books/old.m4b')]));
	const pending = owner.importIntent({ type: 'importPaths', paths: ['/books/new.m4b'] });
	await vi.waitFor(() => expect(capability.analyzeAudioFiles).toHaveBeenCalled());
	owner.setOrderLocked(true);
	finishAnalysis(analyzedFile('/books/new.m4b'));
	await pending;
	expect(owner.view().files.map((file) => file.path)).toEqual(['/books/old.m4b']);
	expect(owner.view().orderLocked).toBe(true);
	expect(owner.view().errorMessage).toBe(
		'Order locked while processing. Wait for completion to add files.',
	);
});
