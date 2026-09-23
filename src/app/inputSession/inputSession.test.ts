import { tauriClient } from '../../lib/tauri/client';
import { titleAudioRequest } from '../../test/fixtures/titleAudio';
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

	it('snapshots imported defaults and edits only the chosen titles without persisting title choices', async () => {
		runtime = createAppRuntime({
			input: fakeInput({
				analyzeAudioFiles: vi.fn(async (paths) => {
					const info = analyzedFile(paths[0]!);
					return { ...info, files: info.files.map((file) => ({ ...file, inputId: file.path })) };
				}),
			}),
		});
		await runtime.initialize();
		await runtime.encoding.reloadCapabilities();
		await runtime.input.importIntent({ type: 'importPaths', paths: ['/books/first.m4b'] });
		const first = runtime.input.view().files[0]!;
		const original = structuredClone(runtime.encoding.audioRequest(first));
		runtime.encoding.select('format', 'mkaOpus');
		runtime.encoding.select('bitrate', '48');
		await runtime.input.importIntent({ type: 'importPaths', paths: ['/books/second.m4b'] });
		const second = runtime.input.view().files[1]!;
		expect(runtime.encoding.audioRequest(first)).toEqual(original);
		expect(runtime.encoding.audioRequest(second)).toMatchObject({
			format: 'mkaOpus',
			intent: 'encode',
			settings: { encoderType: 'opus', bitrateKbps: 48 },
		});
		expect(runtime.encoding.selectionView([first, second]).mixedFields).toContain('format');
		const savedDefaults = structuredClone(runtime.encoding.readDefaults());
		runtime.encoding.selectTitles([first, second], 'format', 'm4b');
		runtime.encoding.selectTitle(first, 'channels', 'mono');
		runtime.encoding.selectTitles([first, second], 'encoder', 'native_aac');
		runtime.encoding.selectTitles([first, second], 'bitrate', '80');
		expect(runtime.encoding.audioRequest(first)).toMatchObject({
			intent: 'encode',
			settings: { channels: 'mono', bitrateKbps: 80 },
		});
		expect(runtime.encoding.audioRequest(second)).toMatchObject({
			intent: 'encode',
			settings: { channels: 'auto', bitrateKbps: 80 },
		});
		expect(runtime.encoding.readDefaults()).toEqual(savedDefaults);
		runtime.encoding.selectTitle(first, 'intent', 'preserve');
		runtime.encoding.applyDefaultsToTitles([second]);
		expect(runtime.encoding.audioRequest(first).intent).toBe('preserve');
		expect(runtime.encoding.audioRequest(second)).toMatchObject({
			format: 'mkaOpus',
			intent: 'encode',
		});
	});

	it('keeps an imported MP3 rate choice when defaults change and only its handling is edited', async () => {
		runtime = createAppRuntime();
		await runtime.initialize();
		await runtime.encoding.reloadCapabilities();
		runtime.encoding.select('format', 'mp3');
		runtime.encoding.select('sampleRate', '22050');
		const file = audioFile('/books/original.mp3');
		runtime.input.replaceSession(sessionWith([file]));
		runtime.encoding.select('sampleRate', '48000');
		runtime.encoding.selectTitle(file, 'intent', 'preserve');
		expect(runtime.encoding.audioRequest(file)).toMatchObject({
			settings: null,
			sampleRate: { explicit: 22050 },
		});
		runtime.encoding.selectTitle(file, 'format', 'm4b');
		expect(runtime.encoding.audioRequest(file).sampleRate).toEqual({ explicit: 22050 });
	});

	it.each([false, true])(
		'waits for stored defaults and respects edits made during loading (%s)',
		async (edited) => {
			const defaults = await tauriClient.getAppSettings();
			defaults.encoderDefaults.format = 'mkaOpus';
			defaults.encoderDefaults.intent = 'preserve';
			defaults.encoderDefaults.settings.encoderType = 'opus';
			defaults.encoderDefaults.settings.bitrateMode = { mode: 'vbr_target' };
			let finish!: (value: typeof defaults) => void;
			const read = vi.spyOn(tauriClient, 'getAppSettings').mockImplementationOnce(
				() =>
					new Promise((resolve) => {
						finish = resolve;
					}),
			);
			const input = fakeInput();
			runtime = createAppRuntime({ input });
			const importing = runtime.input.importIntent({ type: 'pickFiles' });
			await vi.waitFor(() => expect(read).toHaveBeenCalled());
			expect(input.analyzeAudioFiles).not.toHaveBeenCalled();
			if (edited) runtime.encoding.select('format', 'mp3');
			finish(defaults);
			await importing;
			expect(runtime.encoding.audioRequest(runtime.input.view().files[0]!)).toMatchObject({
				format: edited ? 'mp3' : 'mkaOpus',
				intent: 'preserve',
			});
			read.mockRestore();
		},
	);

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

describe('input session selection transitions', () => {
	it('keeps the selection when its requesting workflow aborts during validation', async () => {
		let allow!: (value: boolean) => void;
		const owner = createInputOwner({
			beforeSelectionChange: () =>
				new Promise<boolean>((resolve) => {
					allow = resolve;
				}),
		});
		owner.replaceSession(sessionWith([audioFile('/a'), audioFile('/b')], [1]));
		const request = new AbortController();
		const pending = owner.selectFile({
			index: 0,
			modifiers: { multi: false, range: false },
			signal: request.signal,
		});
		request.abort();
		allow(true);
		expect(await pending).toBe(false);
		expect(owner.session().selectedIndices).toEqual([1]);
	});

	it.each(['select', 'remove'] as const)(
		'resolves %s by file identity after a pending gate and reorder',
		async (action) => {
			let allow!: (value: boolean) => void;
			const owner = createInputOwner({
				beforeSelectionChange: () =>
					new Promise<boolean>((resolve) => {
						allow = resolve;
					}),
			});
			owner.replaceSession(sessionWith([audioFile('/a'), audioFile('/b')], [1]));
			const pending =
				action === 'remove'
					? owner.removeFile(0)
					: owner.selectFile({ index: 0, modifiers: { multi: false, range: false } });
			owner.reorderFiles({ fromIndex: 0, toIndex: 1 });
			allow(true);
			await pending;
			if (action === 'remove') expect(owner.view().files.map((file) => file.path)).toEqual(['/b']);
			else expect(owner.session().selectedIndices).toEqual([1]);
		},
	);

	it('invalidates a pending selection when the session is reset and replaced', async () => {
		let allow!: (value: boolean) => void;
		const owner = createInputOwner({
			beforeSelectionChange: () =>
				new Promise<boolean>((resolve) => {
					allow = resolve;
				}),
		});
		owner.replaceSession(sessionWith([audioFile('/a'), audioFile('/b')], [0]));
		const pending = owner.selectFile({ index: 1, modifiers: { multi: false, range: false } });
		owner.reset();
		owner.replaceSession(sessionWith([audioFile('/a'), audioFile('/b')], [0]));
		allow(true);
		expect(await pending).toBe(false);
		expect(owner.session().selectedIndices).toEqual([0]);
	});

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
		if (action === 'remove') await owner.removeFile(0);
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
		await owner.removeFile(0);
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

describe('per-book audio handling', () => {
	it('keeps explicit choices with book identity through reordering, removal, and reset', async () => {
		const owner = createInputOwner({ capability: fakeInput() });
		const compact = audioFile('/books/prey.m4b', {
			preservation: { canPreserve: true },
		});
		const large = audioFile('/books/large.mp3', {
			preservation: { canPreserve: true },
		});
		owner.replaceSession(sessionWith([compact, large]));
		expect(owner.audioRequest(compact)?.intent).toBeUndefined();
		owner.setAudioRequest(compact, titleAudioRequest({ intent: 'auto' }));
		owner.moveFile({ index: 0, direction: 'down' });
		expect(owner.view().files.map((file) => owner.audioRequest(file)?.intent)).toEqual([
			undefined,
			'auto',
		]);
		owner.setAudioRequest(large, titleAudioRequest({ intent: 'auto' }));
		expect(owner.audioRequest(large)?.intent).toBe('auto');
		owner.setOrderLocked(true);
		owner.setAudioRequest(compact, titleAudioRequest({ intent: 'encode' }));
		expect(owner.audioRequest(compact)?.intent).toBe('auto');
		owner.setOrderLocked(false);
		await owner.removeFile(1);
		owner.replaceSession({ ...owner.session(), fileList: sessionWith([compact, large]).fileList });
		expect(owner.audioRequest(compact)?.intent).toBeUndefined();
		owner.reset();
		owner.replaceSession(sessionWith([large]));
		expect(owner.audioRequest(large)?.intent).toBeUndefined();
	});

	it('stores intent independently of source eligibility and refuses expired identities', () => {
		const owner = createInputOwner({ capability: fakeInput() });
		const wav = audioFile('/books/source.wav', {
			preservation: { canPreserve: false },
		});
		const removed = audioFile('/books/removed.m4b', {
			preservation: { canPreserve: true },
		});
		owner.replaceSession(sessionWith([wav]));
		owner.setAudioRequest(wav, titleAudioRequest({ intent: 'auto' }));
		owner.setAudioRequest(removed, titleAudioRequest({ intent: 'auto' }));
		expect(owner.audioRequest(wav)?.intent).toBe('auto');
		expect(owner.audioRequest(removed)?.intent).toBeUndefined();
	});
});

describe('output title groups', () => {
	it('keeps the metadata anchor and source identities when reordering and splitting a stack', async () => {
		const owner = createInputOwner();
		const files = [
			audioFile('/books/one.m4b'),
			audioFile('/books/two.m4b'),
			audioFile('/books/other.m4b'),
		];
		owner.replaceSession(sessionWith(files, [0, 1]));
		await owner.groupSelected();
		expect(owner.view().files).toEqual([files[0], files[2]]);
		expect(owner.view().sourceFiles).toEqual(files);
		owner.reorderSources(files[0]!, 0, 1);
		expect(owner.view().files[0]).toBe(files[0]);
		expect(owner.sourcesFor(files[0]!)).toEqual([files[1], files[0]]);
		expect(owner.view().totalDurationSeconds).toBe(180);
		await owner.ungroup(files[0]!);
		expect(owner.view().files).toEqual([files[1], files[0], files[2]]);
	});

	it('requires an explicit title-level choice when grouped audio choices disagree', async () => {
		const owner = createInputOwner();
		const files = ['/books/one.m4b', '/books/two.m4b'].map((path) =>
			audioFile(path, { preservation: { canPreserve: true } }),
		);
		owner.replaceSession(sessionWith(files, [0, 1]));
		owner.setAudioRequest(files[1]!, titleAudioRequest({ intent: 'auto' }));
		await owner.groupSelected();
		expect(owner.audioChoiceRequired(files[0]!)).toBe(true);
		owner.setAudioRequest(files[0]!, titleAudioRequest({ intent: 'auto' }));
		expect(owner.audioChoiceRequired(files[0]!)).toBe(false);
		expect(owner.audioRequest(files[0]!)?.intent).toBe('auto');
		await owner.removeFile(0);
		expect(owner.view().sourceFiles).toEqual([]);
		expect(owner.session().audioRequestsByIdentity).toEqual({});
	});

	it('does not group until the metadata draft gate accepts and does not reimport a hidden source', async () => {
		const gate = vi.fn(async () => false);
		const files = [audioFile('/books/one.m4b'), audioFile('/books/two.m4b')];
		const owner = createInputOwner({
			beforeSelectionChange: gate,
			capability: fakeInput({
				analyzeAudioFiles: async () => ({ ...analyzedFile(files[1]!.path), files: [files[1]!] }),
			}),
		});
		owner.replaceSession(sessionWith(files, [0, 1]));
		await owner.groupSelected();
		expect(owner.view().files).toHaveLength(2);
		gate.mockResolvedValue(true);
		await owner.groupSelected();
		await owner.importIntent({ type: 'importPaths', paths: [files[1]!.path] });
		expect(owner.view().files).toHaveLength(1);
		expect(owner.view().sourceFiles).toEqual(files);
	});
});
