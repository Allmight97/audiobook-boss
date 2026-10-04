/**
 * High-value UI workflow smoke test through mocked Tauri boundaries.
 *
 * Keep this as one golden-path composition proof. Owner tests cover their
 * isolated branches; this test protects the user workflow that joins them at
 * the engine submission.
 */
import { cleanup, render, waitFor, screen, within } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createAppRuntime, AppRuntimeProvider } from '../../app/runtime';

import type { AudioFile } from '../../types/audio';
import type { AppSettings } from '../../types/appSettings';
import { createFakeEngine } from '../../test/fixtures/fakeEngine';
import type { OnlineMetadataResult } from '../../types/metadata';
import { App } from '../App';

const native = vi.hoisted(() => ({
	openFiles: vi.fn(),
	openDirectory: vi.fn(),
	getSupportedAudioImportMetadata: vi.fn(),
	readAudioCoverThumbnail: vi.fn(),
	listen: vi.fn(),
	readAudioMetadata: vi.fn(),
	openFile: vi.fn(),
	loadCoverArtFromUrl: vi.fn(),
	listWorkOperations: vi.fn(),
	cancelWorkOperation: vi.fn(),
	openPath: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({ tauriClient: native }));

const INPUT_PATH = '/library/Dune.m4b';
const OUTPUT_DIRECTORY = '/exports/audiobooks';
const COVER_BYTES = [9, 8, 7, 6];

function importedFile(): AudioFile {
	return {
		inputId: 'input-dune',
		path: INPUT_PATH,
		size: 1024,
		duration: 3600,
		format: 'm4b',
		bitrate: 64_000,
		sampleRate: 44100,
		channels: 1,
		codecLabel: 'AAC',
		selectedDecoder: 'FFmpeg',
		tagTitle: 'Dune (old tags)',
		tagArtist: 'Old Author',
		isValid: true,
	};
}

function lookupResult(): OnlineMetadataResult {
	return {
		source: 'audnexus',
		sourceId: 'dune-1965',
		title: 'Dune',
		authors: ['Frank Herbert'],
		narrators: ['George Guidall'],
		series: 'Dune',
		seriesPart: '1',
		subseries: 'Dune Saga',
		subseriesPart: '1',
		description: 'The desert planet Arrakis holds the spice.',
		publishedDate: '1965-08',
		durationSeconds: 3600,
		coverUrl: 'https://covers.example.test/dune.jpg',
		audibleOnly: false,
	};
}

function appSettings(): AppSettings {
	return {
		maxConcurrentJobs: { mode: 'auto' },
		encoderDefaults: {
			format: 'm4b',
			intent: 'auto',
			settings: {
				encoderType: 'auto',
				bitrateKbps: 64,
				bitrateMode: { mode: 'vbr', value: 3 },
				channels: 'auto',
			},
			sampleRate: 'auto',
		},
		outputDefaults: {
			outputNaming: { preset: 'absDefault', includeYear: false },
		},
		startupBehavior: 'rememberLastState',
		keepAwakeWhileWorking: true,
		defaultAcquisitionLane: 'audible',
	};
}

describe('UI Workflow Smoke Test', () => {
	afterEach(() => cleanup());

	it('carries lookup metadata, cover art, output, and encoder choices into a submission', async () => {
		const settings = appSettings();
		native.openFiles.mockResolvedValue([INPUT_PATH]);
		native.openDirectory.mockResolvedValue(OUTPUT_DIRECTORY);
		native.getSupportedAudioImportMetadata.mockResolvedValue({
			formats: [{ extension: 'm4b', label: 'M4B' }],
			extensions: ['m4b'],
			formatsText: 'M4B',
			supportText: 'Supports M4B audio files',
		});
		native.readAudioCoverThumbnail.mockResolvedValue(null);
		native.listen.mockResolvedValue(() => undefined);
		native.readAudioMetadata.mockResolvedValue({
			title: 'Dune (old tags)',
			artist: 'Old Author',
			cover_art: [1, 1, 1],
		});
		native.loadCoverArtFromUrl.mockResolvedValue(COVER_BYTES);
		native.listWorkOperations.mockResolvedValue({ revision: 0, order: [], operations: [] });

		// The engine holds the session: the file's tags, the lookup, and its edits.
		const engine = createFakeEngine(settings);
		engine.analyze = () => [importedFile()];
		engine.tags.set(INPUT_PATH, {
			title: 'Dune (old tags)',
			artist: 'Old Author',
			cover_art: [1, 1, 1],
		});
		// The engine's lookup: one result, and the edits it applies.
		engine.respond = (intent) => {
			if (intent.kind === 'lookupOpen') {
				engine.change((state) => {
					state.lookup = {
						...state.lookup,
						open: true,
						hasSearched: true,
						results: [lookupResult()],
						queuePosition: { index: 0, total: 1, path: INPUT_PATH },
						status: { kind: 'found', count: 1, partial: false, after: null },
					};
				});
				return { kind: 'applied' };
			}
			if (intent.kind === 'lookupApply') {
				engine.seedField('title', 'Dune');
				engine.seedField('author', 'Frank Herbert');
				engine.seedCover(COVER_BYTES);
				engine.change((state) => {
					state.lookup = { ...state.lookup, status: { kind: 'applied', coverFailed: false } };
				});
				return { kind: 'applied' };
			}
			return undefined;
		};
		const runtime = createAppRuntime({ engine });
		const user = userEvent.setup();
		render(() => (
			<AppRuntimeProvider runtime={runtime}>
				<App />
			</AppRuntimeProvider>
		));

		try {
			await waitFor(() => {
				expect(runtime.encoding.view().flavorDisabled).toBe(false);
			});
			await user.click(document.querySelector('[aria-label="Add audio files"]') as HTMLElement);
			await waitFor(() => {
				expect(document.getElementById('meta-title')).toHaveValue('Dune (old tags)');
			});

			await user.click(document.getElementById('metadata-lookup-btn') as HTMLElement);
			const useMetadata = await waitFor(() => {
				const button = document.querySelector<HTMLButtonElement>(
					"#metadata-lookup-results button[data-index='0']",
				);
				if (!button) throw new Error('Lookup result did not render');
				return button;
			});
			await user.click(document.getElementById('metadata-lookup-cover-toggle') as HTMLElement);
			await user.click(useMetadata);
			await waitFor(() => {
				expect(document.getElementById('meta-title')).toHaveValue('Dune');
				expect(document.getElementById('cover-art-img')).not.toHaveClass('hidden');
			});

			await user.click(screen.getByTestId('metadata-lookup-close'));
			await user.click(screen.getByRole('button', { name: /Audio plan for/ }));
			const audioEditor = screen.getByRole('dialog', { name: 'Audio plan' });
			await user.selectOptions(within(audioEditor).getByLabelText('Audio handling'), 'encode');
			// The engine applies each edit; the panel then shows the encoding settings.
			engine.seedTitleAudio('input-dune', {
				format: 'm4b',
				intent: 'encode',
				settings: settings.encoderDefaults.settings,
				sampleRate: 'auto',
			});
			await user.click(await within(audioEditor).findByText(/Encoding settings/));
			await user.selectOptions(within(audioEditor).getByLabelText('Encoder'), 'native_aac');
			const targetBitrate = within(audioEditor).getByLabelText(
				'Bitrate (kbps)',
			) as HTMLInputElement;
			await user.clear(targetBitrate);
			await user.type(targetBitrate, '96');
			await user.tab();
			await user.selectOptions(
				within(audioEditor).getByLabelText('Sample Rate') as HTMLSelectElement,
				'44100',
			);
			await user.selectOptions(
				within(audioEditor).getByLabelText('Channels') as HTMLSelectElement,
				'mono',
			);
			await waitFor(() =>
				expect(engine.sessionIntents).toContainEqual({
					kind: 'setTitleAudio',
					titleIds: ['input-dune'],
					edit: { field: 'channels', value: 'mono' },
				}),
			);
			engine.seedTitleAudio('input-dune', {
				format: 'm4b',
				intent: 'encode',
				settings: {
					encoderType: 'native_aac',
					bitrateKbps: 96,
					bitrateMode: { mode: 'cbr' },
					channels: 'mono',
					nativeAacSpeed: 0,
					faacProfile: 'auto',
				},
				sampleRate: { explicit: 44100 },
			});
			await user.click(document.getElementById('output-dir-browse') as HTMLElement);
			await user.click(document.getElementById('output-abs-include-year') as HTMLElement);
			await waitFor(() => {
				expect(document.getElementById('output-dir-text')).toHaveTextContent(OUTPUT_DIRECTORY);
			});

			await user.click(document.getElementById('process-button') as HTMLElement);
			await waitFor(() => {
				expect(runtime.processing.status().stepText).toContain('Submitted to Work Center.');
			});

			expect(engine.sessionIntents).toContainEqual({
				kind: 'setOutputDirectory',
				directory: OUTPUT_DIRECTORY,
			});
			expect(engine.sessionIntents).toContainEqual({ kind: 'setIncludeYear', includeYear: true });
			expect(engine.sessionIntents[engine.sessionIntents.length - 1]).toEqual({ kind: 'submit' });
			expect(engine.sessionIntents).toContainEqual({
				kind: 'lookupSetReplaceCover',
				replace: true,
			});
			expect(engine.sessionIntents).toContainEqual({ kind: 'lookupApply', index: 0 });
		} finally {
			runtime.dispose();
		}
	});
});
