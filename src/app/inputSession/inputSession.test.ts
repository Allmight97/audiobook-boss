import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo, SupportedAudioImportMetadata } from '../../types/audio';
import { importIntentAtom, inputViewAtom } from './atoms';
import { createTestAppRuntime } from '../runtime/harness';
import type { InputCapability } from '../../lib/tauri/capabilities/input';
import type { AppRuntime } from '../runtime';

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

function fakeInput(overrides: Partial<InputCapability> = {}): InputCapability {
	return {
		openFiles: vi.fn(async () => ['/books/chapter.m4b']),
		openDirectory: vi.fn(async () => '/books'),
		discoverAudioImportPaths: vi.fn(async (paths) => [...paths]),
		analyzeAudioFiles: vi.fn(async () => analyzedFile('/books/chapter.m4b')),
		getSupportedAudioImportMetadata: vi.fn(async () => metadata),
		takeOpenedAudioFiles: vi.fn(async () => []),
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
		runtime.registry.set(importIntentAtom, { type: 'pickFiles' });
		await vi.waitFor(() => {
			expect(runtime?.registry.get(inputViewAtom).files).toHaveLength(1);
		});
		const view = runtime.registry.get(inputViewAtom);
		expect(view.files[0]?.path).toBe('/books/chapter.m4b');
		expect(view.files[0]?.selected).toBe(true);
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
		runtime.registry.set(importIntentAtom, { type: 'importPaths', paths: ['/books/chapter.m4b'] });
		await vi.waitFor(() => {
			expect(runtime?.registry.get(inputViewAtom).errorMessage).toBe(
				'Failed to analyze files. Please try again.',
			);
		});
		expect(runtime.registry.get(inputViewAtom).files).toHaveLength(0);
	});
});
