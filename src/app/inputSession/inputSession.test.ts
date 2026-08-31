import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AudioFile, FileListInfo, SupportedAudioImportMetadata } from '../../types/audio';
import { createTestAppRuntime } from '../runtime/harness';
import type { InputCapability } from '../../lib/tauri/capabilities/input';
import type { AppRuntime } from '../runtime';
import { createInputOwner } from './owner';
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
		runtime = createTestAppRuntime({ input });
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
		runtime = createTestAppRuntime({ input });
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
		runtime = createTestAppRuntime({ input });

		const hydratePromise = runtime.input.hydrateSupportText();
		await runtime.input.importIntent({ type: 'importPaths', paths: ['/books/chapter.m4b'] });
		rejectMetadata!(new Error('lookup failed'));
		await hydratePromise;

		expect(runtime.input.view().files).toHaveLength(1);
		expect(runtime.input.view().files[0]?.path).toBe('/books/chapter.m4b');
	});
});
