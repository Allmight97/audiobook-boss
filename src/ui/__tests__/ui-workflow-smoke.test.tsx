import { cleanup, render, waitFor, screen, within } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createAppRuntime, AppRuntimeProvider } from '../../app/runtime';

import type { AudioFile } from '../../types/audio';
import { createFakeEngine, defaultAppSettings } from '../../test/fixtures/fakeEngine';
import type { OnlineMetadataResult } from '../../types/metadata';
import { App } from '../App';

const native = vi.hoisted(() => ({
	openFiles: vi.fn(),
	openDirectory: vi.fn(),
	listen: vi.fn(),
	openFile: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({ tauriClient: native }));

const INPUT_PATH = '/library/Dune.m4b';
const OUTPUT_DIRECTORY = '/exports/audiobooks';

function importedFile(): AudioFile {
	return {
		inputId: 'input-dune',
		chapters: [],
		path: INPUT_PATH,
		size: 1024,
		duration: 3600,
		format: 'm4b',
		bitrate: 64_000,
		sampleRate: 44100,
		channels: 1,
		codecLabel: 'AAC',
		selectedDecoder: 'FFmpeg',
		label: 'Dune (old tags)',
		artist: 'Old Author',
		chapterPlan: null,
		cueSource: null,
		error: null,
		cover: null,
		isValid: true,
		sourceRemoved: false,
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
		cover: 'remote/0/https%3A%2F%2Fcovers.example.test%2Fdune.jpg',
		audibleOnly: false,
	};
}

describe('UI workflow intents', () => {
	afterEach(() => cleanup());

	it('sends the lookup, cover, audio, and output choices the user made, then submits', async () => {
		native.openFiles.mockResolvedValue([INPUT_PATH]);
		native.openDirectory.mockResolvedValue(OUTPUT_DIRECTORY);
		native.listen.mockResolvedValue(() => undefined);

		const engine = createFakeEngine();
		engine.analyze = () => [importedFile()];
		engine.respond = (intent) => {
			if (intent.kind === 'import') {
				engine.loadTitles(engine.analyze(intent.paths), [0]);
				engine.seedForm({ title: 'Dune (old tags)', author: 'Old Author' });
				engine.seedCover();
				return { kind: 'applied' };
			}
			if (intent.kind === 'lookupClose') {
				engine.change((state) => {
					state.lookup.open = false;
				});
				return { kind: 'applied' };
			}
			if (intent.kind === 'setOutputDirectory') {
				engine.change((state) => {
					state.output.directory = intent.directory;
				});
				return { kind: 'applied' };
			}
			if (intent.kind === 'submit') {
				engine.change((state) => {
					state.output.submission = {
						kind: 'submitted',
						operationId: 'operation-1',
						title: 'Dune',
					};
				});
				return { kind: 'applied' };
			}
			if (intent.kind === 'lookupOpen') {
				engine.change((state) => {
					state.lookup = {
						...state.lookup,
						open: true,
						hasSearched: true,
						results: [lookupResult()],
						queuePosition: { index: 0, total: 1, path: INPUT_PATH, label: 'Dune (old tags)' },
						status: { kind: 'found', count: 1, partial: false, after: null },
					};
				});
				return { kind: 'applied' };
			}
			if (intent.kind === 'lookupApply') {
				engine.seedForm({ title: 'Dune', author: 'Frank Herbert' });
				engine.seedCover();
				engine.change((state) => {
					state.lookup = { ...state.lookup, status: { kind: 'applied', coverFailed: false } };
				});
				return { kind: 'applied' };
			}
			return undefined;
		};
		const runtime = await createAppRuntime({ engine });
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
				expect(screen.getByTestId('cover-art-img')).toBeInTheDocument();
			});

			await user.click(screen.getByTestId('metadata-lookup-close'));
			await waitFor(() => expect(runtime.lookup.view().isOpen).toBe(false));
			await user.click(screen.getByRole('button', { name: /Audio plan for/ }));
			const audioEditor = screen.getByRole('dialog', { name: 'Audio plan' });
			await user.selectOptions(within(audioEditor).getByLabelText('Audio handling'), 'encode');
			engine.seedTitleAudio('input-dune', {
				format: 'm4b',
				intent: 'encode',
				settings: defaultAppSettings().encoderDefaults.settings,
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
			await user.selectOptions(within(audioEditor).getByLabelText('Sample Rate'), '44100');
			await user.selectOptions(within(audioEditor).getByLabelText('Channels'), 'mono');
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
			expect(engine.sessionIntents).toContainEqual({
				kind: 'lookupApply',
				index: 0,
				revision: expect.any(Number),
			});
		} finally {
			runtime.dispose();
		}
	});
});
