import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo } from '../../types/audio';
import type { MetadataCapability } from '../../lib/tauri/capabilities/metadata';
import type { MetadataSaveBatchResult } from '../../types/metadata';
import { createTestAppRuntime } from '../runtime/harness';
import type { AppRuntime } from '../runtime';
import { emptyInputSession } from '../inputSession/types';
import { fakeCoverCapability, stagedCoverView } from '../../test/fixtures/coverCapability';
import {
	getMetadataForFile,
	getMetadataIntentPatchForFile,
	stageMetadataIntentPatch,
} from './index';

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
		expect(getMetadataForFile('/books/alpha.m4b')?.title).toBe('Edited Alpha');
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
		stageMetadataIntentPatch('/books/alpha.m4b', { title: { op: 'set', value: 'Saved' } });
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

	it('drops a late cover URL load when selection changes before the cover view arrives', async () => {
		let resolveUrlLoad: ((value: ReturnType<typeof stagedCoverView>) => void) | undefined;
		const cover = fakeCoverCapability({
			stageFromUrl: vi.fn(
				() =>
					new Promise<ReturnType<typeof stagedCoverView>>((resolve) => {
						resolveUrlLoad = resolve;
					}),
			),
		});
		const metadata = fakeMetadata();
		runtime = createTestAppRuntime({ metadata, cover });
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
		resolveUrlLoad?.(stagedCoverView('cover-late'));
		await loadPromise;
		expect(getMetadataIntentPatchForFile('/books/beta.m4b')?.cover_art).toBeUndefined();
		expect(getMetadataIntentPatchForFile('/books/alpha.m4b')?.cover_art).toBeUndefined();
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
		expect(getMetadataIntentPatchForFile('/books/beta.m4b')?.title).toBeUndefined();
		expect(getMetadataForFile('/books/alpha.m4b')?.title).toBe('Edited Alpha');
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
		stageMetadataIntentPatch('/books/alpha.m4b', { title: { op: 'set', value: 'Saved' } });
		const save = runtime.metadata.save();
		await vi.waitFor(() => {
			expect(metadata.saveMetadataBatch).toHaveBeenCalled();
		});
		expect(runtime.metadata.view().saveInProgress).toBe(true);
		expect(await runtime.metadata.canChangeSelection()).toBe(false);
		releaseSave?.();
		await save;
	});
});
