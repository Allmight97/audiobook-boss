import { afterEach, describe, expect, it, vi } from 'vitest';
import type { MetadataCapability } from '../../lib/tauri/capabilities/metadata';
import type { FileListInfo } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import type {
	MetadataIntentPatch,
	MetadataIntentValidationResult,
} from '../../types/metadataIntent';
import { emptyInputSession } from '../inputSession/types';
import { createAppRuntime, type AppRuntime } from '../runtime';

// Edit intent survives from the form to the save request unchanged: untouched
// fields stay untouched, explicit clears clear, and failures explain themselves.

type Book = { readonly path: string; readonly valid?: boolean; readonly tags: AudiobookMetadata };

function sessionOf(books: ReadonlyArray<Book>, selected: number[]) {
	const files = books.map((book) => ({
		path: book.path,
		inputId: book.path,
		isValid: book.valid ?? true,
		duration: 60,
		size: 1024,
		format: 'm4b',
	}));
	const fileList: FileListInfo = {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: files.length * 60,
		totalSize: files.length * 1024,
		validCount: files.filter((file) => file.isValid).length,
		invalidCount: files.filter((file) => !file.isValid).length,
	};
	return {
		...emptyInputSession(),
		fileList,
		selectedIndices: selected,
		selectedAnchor: selected[0],
	};
}

function capability(
	books: ReadonlyArray<Book>,
	overrides: Partial<MetadataCapability> = {},
): MetadataCapability {
	return {
		readAudioMetadata: vi.fn(async (path: string) => ({
			...books.find((book) => book.path === path)?.tags,
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

function savedPatches(metadata: MetadataCapability): Record<string, MetadataIntentPatch> {
	return Object.fromEntries(
		vi
			.mocked(metadata.saveMetadataBatch)
			.mock.calls.flatMap(([items]) => items.map((item) => [item.filePath, item.metadataPatch])),
	);
}

const genre = { genre: { op: 'set', value: 'Mystery' } };
const alpha: Book = {
	path: '/books/alpha.m4b',
	tags: {
		title: 'Alpha',
		album: 'Alpha',
		artist: 'Shared Author',
		composer: 'Alpha Reader',
		genre: 'Fantasy',
		date: '2001',
		series: 'Saga',
		series_part: '1',
		subseries: 'Arc',
		subseries_part: '2',
		description: 'About alpha',
		cover_art: [7, 7, 7],
	},
};
const beta: Book = {
	path: '/books/beta.m4b',
	tags: { ...alpha.tags, title: 'Beta', album: 'Beta', genre: 'Horror', date: '2002' },
};

describe('metadata edit intent', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
	});

	async function open(
		books: ReadonlyArray<Book>,
		selected: number[],
		metadata: MetadataCapability,
	) {
		runtime = createAppRuntime({ metadata });
		runtime.input.replaceSession(sessionOf(books, selected));
		await runtime.metadata.hydrateSelection(null);
		return runtime;
	}

	it.each([
		{ inputId: 'meta-author', actionId: 'meta-author-action', shown: 'Shared Author' },
		{ inputId: 'meta-genre', actionId: 'meta-genre-action', shown: '' },
		{ inputId: 'meta-year', actionId: 'meta-year-action', shown: '' },
		{ inputId: 'meta-title', actionId: 'meta-title-action', shown: '' },
	])(
		'Keep after Blank on $inputId revokes the bulk clear',
		async ({ inputId, actionId, shown }) => {
			const metadata = capability([alpha, beta]);
			const app = await open([alpha, beta], [0, 1], metadata);
			app.metadata.setFieldAction({ actionId, action: 'blank' });
			app.metadata.setFieldAction({ actionId, action: 'keep' });
			expect(app.metadata.view().form.fields[inputId as 'meta-title']).toMatchObject({
				value: shown,
				dirty: false,
			});
			app.metadata.setFieldValue({ inputId: 'meta-genre', value: 'Mystery' });
			await app.metadata.save();
			expect(savedPatches(metadata)).toEqual({ [alpha.path]: genre, [beta.path]: genre });
		},
	);

	it('Blank clears the field and its album mirror on every selected title', async () => {
		const metadata = capability([alpha, beta]);
		const app = await open([alpha, beta], [0, 1], metadata);
		app.metadata.setFieldAction({ actionId: 'meta-title-action', action: 'blank' });
		await app.metadata.save();
		const cleared = { title: { op: 'clear' }, album: { op: 'clear' } };
		expect(savedPatches(metadata)).toEqual({ [alpha.path]: cleared, [beta.path]: cleared });
	});

	it('an invalid co-selected input does not turn untouched fields into clears', async () => {
		const broken: Book = { path: '/books/broken.m4b', valid: false, tags: {} };
		const metadata = capability([alpha, broken]);
		const app = await open([alpha, broken], [0, 1], metadata);
		app.metadata.setFieldValue({ inputId: 'meta-genre', value: 'Mystery' });
		await app.metadata.save();
		expect(savedPatches(metadata)).toEqual({ [alpha.path]: genre });
	});

	it('inherited values are neither rewritten nor revalidated by an unrelated edit', async () => {
		const odd: Book = { path: alpha.path, tags: { ...alpha.tags, series_part: '7/8' } };
		const metadata = capability([odd], {
			validateMetadataIntentPatch: vi.fn(async (patch) => ({
				isValid: !patch.series_part,
				metadataPatch: patch,
				fieldErrors: patch.series_part
					? [
							{
								field: 'series_part' as const,
								code: 'series_part_contains_slash' as const,
								message: 'Bad part',
							},
						]
					: [],
			})),
		});
		const app = await open([odd], [0], metadata);
		app.metadata.setFieldValue({ inputId: 'meta-genre', value: 'Mystery' });
		await app.metadata.save();
		expect(savedPatches(metadata)).toEqual({ [alpha.path]: genre });
	});

	it('a text edit does not re-stage the unchanged cover', async () => {
		const metadata = capability([alpha]);
		const app = await open([alpha], [0], metadata);
		app.metadata.setFieldValue({ inputId: 'meta-title', value: 'Renamed' });
		await app.metadata.save();
		expect(savedPatches(metadata)).toEqual({
			[alpha.path]: {
				title: { op: 'set', value: 'Renamed' },
				album: { op: 'set', value: 'Renamed' },
			},
		});
	});

	it('Lookup queue apply with the existing cover leaves cover intent noop', async () => {
		const metadata = capability([alpha, beta], {
			searchOnlineMetadata: vi.fn(async () => ({
				results: [
					{
						source: 'audnexus' as const,
						sourceId: '1',
						title: 'Found',
						authors: [],
						narrators: [],
						audibleOnly: false,
						coverUrl: 'https://example.com/cover.jpg',
					},
				],
				diagnostics: [],
			})),
		});
		const app = await open([alpha, beta], [0, 1], metadata);
		await app.lookup.run({ type: 'open' });
		await app.lookup.run({ type: 'applyResult', index: 0 });
		expect(app.lookup.view().statusMessage).toContain('Metadata applied.');
		await app.metadata.save();
		expect(savedPatches(metadata)[alpha.path]).toEqual({
			title: { op: 'set', value: 'Found' },
			album: { op: 'set', value: 'Found' },
		});
	});

	it('a validation failure that returns after a newer edit is not published', async () => {
		let reject!: () => void;
		const metadata = capability([alpha, beta], {
			validateMetadataIntentPatch: vi.fn(
				(patch) =>
					new Promise<MetadataIntentValidationResult>((resolve) => {
						reject = () =>
							resolve({
								isValid: false,
								metadataPatch: patch,
								fieldErrors: [
									{ field: 'date', code: 'publication_date_syntax', message: 'Invalid date' },
								],
							});
					}),
			),
		});
		const app = await open([alpha, beta], [0], metadata);
		app.metadata.setFieldValue({ inputId: 'meta-year', value: 'soon' });
		const selecting = app.input.selectFile({ index: 1, modifiers: { multi: false, range: false } });
		await vi.waitFor(() => expect(reject).toBeDefined());
		app.metadata.setFieldValue({ inputId: 'meta-year', value: '2020' });
		reject();
		expect(await selecting).toBe(false);
		expect(app.metadata.view().statusMessage).not.toBe('Invalid date');
		expect(app.metadata.view().form.fields['meta-year'].value).toBe('2020');
	});

	it('a rejected save explains itself and keeps the pending edit', async () => {
		const metadata = capability([alpha], {
			saveMetadataBatch: vi.fn(async () => {
				throw { code: 'io_error', category: 'io', message: 'The disk is read-only.' };
			}),
		});
		const app = await open([alpha], [0], metadata);
		app.metadata.setFieldValue({ inputId: 'meta-genre', value: 'Mystery' });
		await app.metadata.save();
		expect(app.metadata.view()).toMatchObject({
			saveInProgress: false,
			statusMessage: 'The disk is read-only.',
		});
		await app.metadata.save();
		expect(vi.mocked(metadata.saveMetadataBatch).mock.calls[1]?.[0]).toEqual([
			{ filePath: alpha.path, metadataPatch: genre },
		]);
	});

	it('processing stage reports edits that have no valid target', async () => {
		const broken: Book = { path: '/books/broken.m4b', valid: false, tags: {} };
		const app = await open([broken], [0], capability([broken]));
		app.metadata.setFieldValue({ inputId: 'meta-title', value: 'Orphan' });
		expect(await app.metadata.stageCurrentSelection()).toEqual({ status: 'noTarget' });
	});

	it('processing stage reports invalid edits and stages nothing', async () => {
		const metadata = capability([alpha], {
			validateMetadataIntentPatch: vi.fn(async (patch) => ({
				isValid: false,
				metadataPatch: patch,
				fieldErrors: [
					{ field: 'date' as const, code: 'publication_date_syntax' as const, message: 'Bad date' },
				],
			})),
		});
		const app = await open([alpha], [0], metadata);
		app.metadata.setFieldValue({ inputId: 'meta-year', value: 'soon' });
		expect(await app.metadata.stageCurrentSelection()).toEqual({
			status: 'invalid',
			message: 'Bad date',
		});
		expect(await app.metadata.intentsForProcess([alpha.path])).toBeNull();
	});

	it('processing stage carries a cover clear without text edits', async () => {
		const app = await open([alpha], [0], capability([alpha]));
		app.metadata.clearCoverArt();
		expect(await app.metadata.stageCurrentSelection()).toEqual({ status: 'staged' });
		expect(await app.metadata.intentsForProcess([alpha.path])).toEqual({
			[alpha.path]: { cover_art: { op: 'clear' } },
		});
	});

	it('Keep after a processing stage keeps the staged value and its intent in step', async () => {
		const app = await open([alpha, beta], [0, 1], capability([alpha, beta]));
		app.metadata.setFieldValue({ inputId: 'meta-genre', value: 'Mystery' });
		expect(await app.metadata.stageCurrentSelection()).toEqual({ status: 'staged' });
		app.metadata.setFieldAction({ actionId: 'meta-genre-action', action: 'blank' });
		app.metadata.setFieldAction({ actionId: 'meta-genre-action', action: 'keep' });
		expect(app.metadata.view().form.fields['meta-genre']).toMatchObject({
			value: 'Mystery',
			dirty: false,
		});
		expect(await app.metadata.intentsForProcess([alpha.path, beta.path])).toEqual({
			[alpha.path]: genre,
			[beta.path]: genre,
		});
	});
});
