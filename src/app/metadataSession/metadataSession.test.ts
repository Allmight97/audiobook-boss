import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo } from '../../types/audio';
import type { MetadataCapability } from '../../lib/tauri/capabilities/metadata';
import { createTestAppRuntime } from '../runtime/harness';
import type { AppRuntime } from '../runtime';
import { inputSessionAtom } from '../inputSession/atoms';
import { emptyInputSession } from '../inputSession/types';
import {
	getMetadataForFile,
	hydrateMetadataSelectionAtom,
	metadataEditorAtom,
	saveMetadataAtom,
	setMetadataFieldValueAtom,
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
		runtime.registry.set(inputSessionAtom, {
			...emptyInputSession(),
			fileList: list(files),
			selectedIndices: [0],
			selectedAnchor: 0,
		});
		runtime.registry.set(hydrateMetadataSelectionAtom, null);
		await vi.waitFor(() => {
			expect(runtime?.registry.get(metadataEditorAtom).form.fields['meta-title'].value).toBe(
				'Alpha',
			);
		});
		expect(metadata.readAudioMetadata).toHaveBeenCalledWith('/books/alpha.m4b');
	});

	it('commits a dirty title onto the previous file before hydrating the next selection', async () => {
		const metadata = fakeMetadata();
		runtime = createTestAppRuntime({ metadata });
		const files = [file('/books/alpha.m4b', 'Alpha'), file('/books/beta.m4b', 'Beta')];
		runtime.registry.set(inputSessionAtom, {
			...emptyInputSession(),
			fileList: list(files),
			selectedIndices: [0],
			selectedAnchor: 0,
		});
		runtime.registry.set(hydrateMetadataSelectionAtom, null);
		await vi.waitFor(() => {
			expect(runtime?.registry.get(metadataEditorAtom).form.fields['meta-title'].value).toBe(
				'Alpha',
			);
		});
		runtime.registry.set(setMetadataFieldValueAtom, {
			inputId: 'meta-title',
			value: 'Edited Alpha',
		});
		runtime.registry.set(inputSessionAtom, {
			...runtime.registry.get(inputSessionAtom),
			selectedIndices: [1],
			selectedAnchor: 1,
		});
		runtime.registry.set(hydrateMetadataSelectionAtom, null);
		await vi.waitFor(() => {
			expect(runtime?.registry.get(metadataEditorAtom).form.fields['meta-title'].value).toBe(
				'Beta',
			);
		});
		expect(getMetadataForFile('/books/alpha.m4b')?.title).toBe('Edited Alpha');
	});

	it('saves staged intent through the metadata capability', async () => {
		const metadata = fakeMetadata();
		runtime = createTestAppRuntime({ metadata });
		const files = [file('/books/alpha.m4b', 'Alpha')];
		runtime.registry.set(inputSessionAtom, {
			...emptyInputSession(),
			fileList: list(files),
			selectedIndices: [0],
			selectedAnchor: 0,
		});
		stageMetadataIntentPatch('/books/alpha.m4b', { title: { op: 'set', value: 'Saved' } });
		runtime.registry.set(saveMetadataAtom, undefined);
		await vi.waitFor(() => {
			expect(metadata.saveMetadataBatch).toHaveBeenCalled();
		});
		expect(metadata.saveMetadataBatch).toHaveBeenCalledWith([
			{
				filePath: '/books/alpha.m4b',
				metadataPatch: { title: { op: 'set', value: 'Saved' } },
			},
		]);
	});
});
