import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo } from '../../types/audio';
import type { MetadataCapability } from '../../lib/tauri/capabilities/metadata';
import type { MetadataSaveBatchResult } from '../../types/metadata';
import { createTestAppRuntime } from '../runtime/harness';
import type { AppRuntime } from '../runtime';
import { emptyInputSession } from '../inputSession/types';
import * as metadataSessionApi from './index';

function file(path: string, title: string): FileListInfo['files'][number] {
	return {
		path,
		inputId: path,
		isValid: true,
		duration: 60,
		size: 1024,
		format: 'm4b',
		tagTitle: title,
	};
}

function list(files: FileListInfo['files']): FileListInfo {
	return {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: files.length * 60,
		totalSize: files.length * 1024,
		validCount: files.length,
		invalidCount: 0,
	};
}

function fakeMetadata(overrides: Partial<MetadataCapability> = {}): MetadataCapability {
	return {
		readAudioMetadata: vi.fn(async (filePath) => ({
			title: filePath.includes('alpha') ? 'Alpha' : 'Beta',
			artist: 'Author',
		})),
		validateMetadataIntentPatch: vi.fn(async (patch) => ({
			isValid: true,
			metadataPatch: patch,
			fieldErrors: [],
		})),
		saveMetadataBatch: vi.fn(async (items: ReadonlyArray<{ filePath: string }>) => ({
			results: items.map((item, inputIndex) => ({
				inputIndex,
				filePath: item.filePath,
				status: 'success' as const,
				message: 'ok',
			})),
			summary: {
				succeeded: items.length,
				failed: 0,
				cancelled: 0,
				skipped: 0,
				total: items.length,
			},
		})),
		openFile: vi.fn(async () => null),
		loadCoverArtFile: vi.fn(async () => [1, 2, 3]),
		loadCoverArtFromUrl: vi.fn(async () => [1, 2, 3]),
		searchOnlineMetadata: vi.fn(async () => ({ results: [], diagnostics: [] })),
		...overrides,
	};
}

describe('metadata session selection and save', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
	});

	it('hydrates a single selection from native metadata reads', async () => {
		const metadata = fakeMetadata();
		runtime = createTestAppRuntime({ metadata });
		const files = [file('/books/alpha.m4b', 'Alpha')];
		runtime.input.replaceSession({
			...emptyInputSession(),
			fileList: list(files),
			selectedIndices: [0],
			selectedAnchor: 0,
		});
		await runtime.metadata.hydrateSelection(null);
		expect(runtime.metadata.view().form.fields['meta-title'].value).toBe('Alpha');
		expect(metadata.readAudioMetadata).toHaveBeenCalledWith('/books/alpha.m4b');
	});

	it('commits a dirty title onto the previous file before hydrating the next selection', async () => {
		const metadata = fakeMetadata();
		runtime = createTestAppRuntime({ metadata });
		const files = [file('/books/alpha.m4b', 'Alpha'), file('/books/beta.m4b', 'Beta')];
		runtime.input.replaceSession({
			...emptyInputSession(),
			fileList: list(files),
			selectedIndices: [0],
			selectedAnchor: 0,
		});
		await runtime.metadata.hydrateSelection(null);
		expect(runtime.metadata.view().form.fields['meta-title'].value).toBe('Alpha');
		runtime.metadata.setFieldValue({
			inputId: 'meta-title',
			value: 'Edited Alpha',
		});
		runtime.input.replaceSession({
			...runtime.input.session(),
			selectedIndices: [1],
			selectedAnchor: 1,
		});
		await runtime.metadata.hydrateSelection(null);
		expect(runtime.metadata.view().form.fields['meta-title'].value).toBe('Beta');
		expect(runtime.metadata.readCached('/books/alpha.m4b')?.title).toBe('Edited Alpha');
	});

	it('saves staged intent through the metadata capability', async () => {
		const metadata = fakeMetadata();
		runtime = createTestAppRuntime({ metadata });
		const files = [file('/books/alpha.m4b', 'Alpha')];
		runtime.input.replaceSession({
			...emptyInputSession(),
			fileList: list(files),
			selectedIndices: [0],
			selectedAnchor: 0,
		});
		runtime.metadata.stageIntent('/books/alpha.m4b', { title: { op: 'set', value: 'Saved' } });
		await runtime.metadata.save();
		expect(metadata.saveMetadataBatch).toHaveBeenCalled();
		expect(metadata.saveMetadataBatch).toHaveBeenCalledWith([
			{
				filePath: '/books/alpha.m4b',
				metadataPatch: { title: { op: 'set', value: 'Saved' } },
			},
		]);
	});

	it('fails hydration when draft validation transport fails', async () => {
		const metadata = fakeMetadata({
			validateMetadataIntentPatch: vi.fn(async () => {
				throw new Error('lookup transport down');
			}),
		});
		runtime = createTestAppRuntime({ metadata });
		const files = [file('/books/alpha.m4b', 'Alpha'), file('/books/beta.m4b', 'Beta')];
		runtime.input.replaceSession({
			...emptyInputSession(),
			fileList: list(files),
			selectedIndices: [0],
			selectedAnchor: 0,
		});
		await runtime.metadata.hydrateSelection(null);
		runtime.metadata.setFieldValue({
			inputId: 'meta-title',
			value: 'Edited Alpha',
		});
		runtime.input.replaceSession({
			...runtime.input.session(),
			selectedIndices: [1],
			selectedAnchor: 1,
		});
		await runtime.metadata.hydrateSelection(null);
		expect(runtime.metadata.view().form.fields['meta-title'].value).toBe('Edited Alpha');
		expect(runtime.metadata.view().statusMessage).toMatch(/validate metadata/i);
		expect(metadata.readAudioMetadata).not.toHaveBeenCalledWith('/books/beta.m4b');
	});

	it('drops a late cover URL load when selection changes before the bytes arrive', async () => {
		let resolveUrlLoad: ((value: number[]) => void) | undefined;
		const metadata = fakeMetadata({
			loadCoverArtFromUrl: vi.fn(
				() =>
					new Promise<number[]>((resolve) => {
						resolveUrlLoad = resolve;
					}),
			),
		});
		runtime = createTestAppRuntime({ metadata });
		const files = [file('/books/alpha.m4b', 'Alpha'), file('/books/beta.m4b', 'Beta')];
		runtime.input.replaceSession({
			...emptyInputSession(),
			fileList: list(files),
			selectedIndices: [0],
			selectedAnchor: 0,
		});
		await runtime.metadata.hydrateSelection(null);
		const loadPromise = runtime.metadata.loadCoverArtFromUrl('https://example.com/cover.jpg');
		runtime.input.replaceSession({
			...runtime.input.session(),
			selectedIndices: [1],
			selectedAnchor: 1,
		});
		await runtime.metadata.hydrateSelection(null);
		resolveUrlLoad?.([9, 9, 9]);
		await loadPromise;
		expect(await runtime.metadata.intentsForProcess(['/books/beta.m4b'])).toBeNull();
		expect(await runtime.metadata.intentsForProcess(['/books/alpha.m4b'])).toBeNull();
		expect(runtime.metadata.view().cover.hasCustomCoverArt).toBe(false);
	});

	it('does not commit process staging when selection changes during validation', async () => {
		let releaseValidate: (() => void) | undefined;
		let validateCalls = 0;
		const metadata = fakeMetadata({
			validateMetadataIntentPatch: vi.fn(async (patch) => {
				validateCalls += 1;
				if (validateCalls === 1) {
					await new Promise<void>((resolve) => {
						releaseValidate = resolve;
					});
				}
				return {
					isValid: true,
					metadataPatch: patch,
					fieldErrors: [],
				};
			}),
		});
		runtime = createTestAppRuntime({ metadata });
		const files = [file('/books/alpha.m4b', 'Alpha'), file('/books/beta.m4b', 'Beta')];
		runtime.input.replaceSession({
			...emptyInputSession(),
			fileList: list(files),
			selectedIndices: [0],
			selectedAnchor: 0,
		});
		await runtime.metadata.hydrateSelection(null);
		runtime.metadata.setFieldValue({
			inputId: 'meta-title',
			value: 'Edited Alpha',
		});
		const stagePromise = runtime.metadata.stageCurrentSelectionForProcess();
		runtime.input.replaceSession({
			...runtime.input.session(),
			selectedIndices: [1],
			selectedAnchor: 1,
		});
		await runtime.metadata.hydrateSelection(null);
		releaseValidate?.();
		expect(await stagePromise).toBe(false);
		expect(await runtime.metadata.intentsForProcess(['/books/beta.m4b'])).toBeNull();
		expect(runtime.metadata.readCached('/books/alpha.m4b')?.title).toBe('Edited Alpha');
	});

	it('blocks selection change while a metadata save is in progress', async () => {
		let releaseSave: (() => void) | undefined;
		const metadata = fakeMetadata({
			saveMetadataBatch: vi.fn(
				() =>
					new Promise<MetadataSaveBatchResult>((resolve) => {
						releaseSave = () =>
							resolve({
								results: [
									{
										inputIndex: 0,
										filePath: '/books/alpha.m4b',
										status: 'success',
										message: 'ok',
									},
								],
								summary: {
									succeeded: 1,
									failed: 0,
									cancelled: 0,
									skipped: 0,
									total: 1,
								},
							});
					}),
			),
		});
		runtime = createTestAppRuntime({ metadata });
		const files = [file('/books/alpha.m4b', 'Alpha'), file('/books/beta.m4b', 'Beta')];
		runtime.input.replaceSession({
			...emptyInputSession(),
			fileList: list(files),
			selectedIndices: [0],
			selectedAnchor: 0,
		});
		runtime.metadata.stageIntent('/books/alpha.m4b', { title: { op: 'set', value: 'Saved' } });
		const save = runtime.metadata.save();
		await vi.waitFor(() => {
			expect(metadata.saveMetadataBatch).toHaveBeenCalled();
		});
		expect(runtime.metadata.view().saveInProgress).toBe(true);
		expect(await runtime.metadata.canChangeSelection()).toBe(false);
		releaseSave?.();
		await save;
	});

	it('drops cached tags when a path is removed so re-import reads native metadata again', async () => {
		let alphaTitle = 'Alpha';
		const readAudioMetadata = vi.fn(async (filePath: string) => ({
			title: filePath.includes('alpha') ? alphaTitle : 'Beta',
			artist: 'Author',
			cover_art: [1],
		}));
		const metadata = fakeMetadata({ readAudioMetadata });
		runtime = createTestAppRuntime({ metadata });
		const alpha = file('/books/alpha.m4b', 'Alpha');
		const beta = file('/books/beta.m4b', 'Beta');
		runtime.input.replaceSession({
			...emptyInputSession(),
			fileList: list([alpha, beta]),
			selectedIndices: [0],
			selectedAnchor: 0,
		});
		await runtime.metadata.hydrateSelection(null);
		expect(runtime.metadata.readCached('/books/alpha.m4b')?.title).toBe('Alpha');

		runtime.input.removeFile(0);
		await runtime.metadata.hydrateSelection(null);
		expect(runtime.metadata.readCached('/books/alpha.m4b')).toBeUndefined();

		alphaTitle = 'Alpha From Disk';
		readAudioMetadata.mockClear();
		runtime.input.replaceSession({
			...emptyInputSession(),
			fileList: list([alpha, beta]),
			selectedIndices: [0],
			selectedAnchor: 0,
		});
		await runtime.metadata.hydrateSelection(null);
		expect(readAudioMetadata).toHaveBeenCalledWith('/books/alpha.m4b');
		expect(runtime.metadata.readCached('/books/alpha.m4b')?.title).toBe('Alpha From Disk');
		expect(runtime.metadata.view().form.fields['meta-title'].value).toBe('Alpha From Disk');
	});

	it('intentsForProcess loads native tags when cache only contains cover art', async () => {
		const readAudioMetadata = vi.fn(async (filePath: string) => {
			const count = readAudioMetadata.mock.calls.filter(([path]) => path === filePath).length;
			if (count === 1) {
				return { cover_art: [1, 2, 3] };
			}
			return { title: 'Loaded From Disk' };
		});
		const metadata = fakeMetadata({ readAudioMetadata });
		runtime = createTestAppRuntime({ metadata });
		runtime.input.replaceSession({
			...emptyInputSession(),
			fileList: list([file('/books/alpha.m4b', 'Alpha')]),
			selectedIndices: [0],
			selectedAnchor: 0,
		});
		await runtime.metadata.hydrateSelection(null);
		expect(runtime.metadata.readCached('/books/alpha.m4b')).toEqual({ cover_art: [1, 2, 3] });

		await expect(runtime.metadata.intentsForProcess(['/books/alpha.m4b'])).resolves.toBeNull();
		expect(runtime.metadata.readCached('/books/alpha.m4b')).toEqual({ title: 'Loaded From Disk' });
		expect(readAudioMetadata).toHaveBeenCalledTimes(2);
	});

	it('does not export process-global cache helpers', () => {
		expect(metadataSessionApi).not.toHaveProperty('cacheMetadataForFile');
		expect(metadataSessionApi).not.toHaveProperty('getMetadataForFile');
		expect(metadataSessionApi).not.toHaveProperty('getMetadataIntentPatchForFile');
		expect(metadataSessionApi).not.toHaveProperty('stageMetadataIntentPatch');
		expect(metadataSessionApi).not.toHaveProperty('collectActionableMetadataIntent');
		expect(metadataSessionApi).not.toHaveProperty('clearPendingMetadataForFile');
		expect(metadataSessionApi).not.toHaveProperty('removeMetadataForFile');
		expect(metadataSessionApi).not.toHaveProperty('clearMetadataSession');
		expect(metadataSessionApi).not.toHaveProperty('isUsableMetadataCache');
	});
});
