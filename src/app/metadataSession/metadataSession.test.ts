import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo } from '../../types/audio';
import type { MetadataCapability } from '../../lib/tauri/capabilities/metadata';
import type { MetadataSaveBatchResult } from '../../types/metadata';
import { createTestAppRuntime } from '../runtime/harness';
import type { AppRuntime } from '../runtime';
import { emptyInputSession } from '../inputSession/types';
import { getMetadataForFile, stageMetadataIntentPatch } from './index';

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
