import { flush } from 'solid-js';
import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';

import { AppSettingsDialogView } from './AppSettingsDialogView';

describe('AppSettingsDialogView', () => {
	let runtime: AppRuntime | undefined;
	let engine: FakeEngine;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
	});

	async function renderOpenDialog(
		prepare: (engine: FakeEngine) => void = () => {},
	): Promise<AppRuntime> {
		engine = createFakeEngine();
		prepare(engine);
		runtime = createAppRuntime({ engine });
		await runtime.initialize();
		runtime.settings.openDialog();
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<AppSettingsDialogView />
			</AppRuntimeProvider>
		));
		return runtime;
	}

	function sent(kind: string): number {
		return engine.settingsIntents.filter((intent) => intent.kind === kind).length;
	}

	it('requires a second activation before resetting all settings', async () => {
		await renderOpenDialog();

		fireEvent.click(screen.getByTestId('app-settings-reset'));
		flush();
		expect(sent('reset')).toBe(0);
		expect(screen.getByTestId('app-settings-reset-confirm-prompt')).toBeInTheDocument();

		fireEvent.click(screen.getByTestId('app-settings-reset-confirm'));
		flush();
		await vi.waitFor(() => expect(sent('reset')).toBe(1));
	});

	it('keeps Reset disabled and sends one reset while the reset is saving', async () => {
		await renderOpenDialog();
		const release = { done: () => {} };
		engine.respondSettings = (intent) =>
			intent.kind === 'reset'
				? new Promise((resolve) => {
						release.done = () => resolve(undefined);
					})
				: undefined;

		fireEvent.click(screen.getByTestId('app-settings-reset'));
		flush();
		fireEvent.click(screen.getByTestId('app-settings-reset-confirm'));
		flush();
		await vi.waitFor(() => expect(sent('reset')).toBe(1));

		const reset = screen.getByTestId('app-settings-reset');
		expect(reset).toBeDisabled();
		fireEvent.click(reset);
		flush();
		expect(screen.queryByTestId('app-settings-reset-confirm-prompt')).not.toBeInTheDocument();
		expect(sent('reset')).toBe(1);
		release.done();
	});

	it('shows an automatic save failure and retries without discarding the accepted choice', async () => {
		await renderOpenDialog();
		engine.change((state) => {
			state.audio.defaults.choice.intent = 'encode';
		});
		flush();
		fireEvent.change(screen.getByTestId('encoder-select'), {
			target: { value: 'faac' },
		});
		flush();
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'setDefaultAudio',
				edit: { field: 'encoder', value: 'faac' },
			}),
		);
		engine.change((state) => {
			state.audio.defaults.choice.encoder = 'faac';
			state.audio.defaults.facts.effectiveEncoder = 'faac';
		});
		const faac = {
			...engine.settings().settings!.encoderDefaults,
			settings: {
				...engine.settings().settings!.encoderDefaults.settings,
				encoderType: 'faac' as const,
			},
		};
		engine.seedAppSettings(
			{ encoderDefaults: faac },
			{ code: 'io_error', category: 'io', message: 'Disk full' },
		);
		await vi.waitFor(() =>
			expect(screen.getByRole('button', { name: 'Retry save' })).toBeInTheDocument(),
		);
		expect(screen.getByRole('status')).toHaveTextContent(
			'Your current choices still apply for this session. Disk full',
		);
		expect(screen.getByTestId('encoder-select')).toHaveValue('faac');
		engine.respondSettings = () => {
			engine.seedAppSettings({});
			return undefined;
		};
		fireEvent.click(screen.getByRole('button', { name: 'Retry save' }));
		flush();
		await vi.waitFor(() =>
			expect(screen.queryByText("Settings haven't been saved")).not.toBeInTheDocument(),
		);
		expect(screen.getByTestId('encoder-select')).toHaveValue('faac');
	});

	it('returns to idle when the confirm step is cancelled', async () => {
		await renderOpenDialog();

		fireEvent.click(screen.getByTestId('app-settings-reset'));
		flush();
		fireEvent.click(screen.getByTestId('app-settings-reset-cancel'));
		flush();

		expect(sent('reset')).toBe(0);
		expect(screen.queryByTestId('app-settings-reset-confirm-prompt')).not.toBeInTheDocument();
		expect(screen.getByTestId('app-settings-reset')).toBeInTheDocument();
	});

	it('returns to idle when a click lands outside the reset row', async () => {
		await renderOpenDialog();

		fireEvent.click(screen.getByTestId('app-settings-reset'));
		flush();
		fireEvent.click(screen.getByRole('heading', { name: 'App Settings' }));
		flush();

		expect(screen.queryByTestId('app-settings-reset-confirm-prompt')).not.toBeInTheDocument();
	});

	it('records the default acquisition lane when the Indexer radio is selected', async () => {
		await renderOpenDialog();
		engine.respondSettings = () => {
			engine.seedAppSettings({ defaultAcquisitionLane: 'indexer' });
			return undefined;
		};

		fireEvent.click(screen.getByTestId('app-settings-default-lane-indexer'));
		flush();

		await vi.waitFor(() => expect(runtime!.settings.defaultAcquisitionLane()).toBe('indexer'));
		expect(engine.settingsIntents).toContainEqual({
			kind: 'remember',
			encoderDefaults: null,
			outputDefaults: null,
			defaultAcquisitionLane: 'indexer',
		});
	});

	it('sends the AAC decoder choice and shows the decoder the engine holds', async () => {
		await renderOpenDialog();

		expect(screen.getByTestId('app-settings-aac-decoder-auto')).toBeChecked();
		engine.respondSettings = () => {
			engine.seedAppSettings({ aacDecoder: 'faad' });
			return undefined;
		};
		fireEvent.click(screen.getByTestId('app-settings-aac-decoder-faad'));
		flush();

		await vi.waitFor(() => {
			expect(screen.getByTestId('app-settings-aac-decoder-faad')).toBeChecked();
		});
		expect(engine.settingsIntents).toContainEqual({ kind: 'setAacDecoder', decoder: 'faad' });
	});

	it('keeps a choice that could not be saved checked and reports why', async () => {
		await renderOpenDialog();
		const awake = screen.getByTestId('app-settings-keep-awake-checkbox');
		expect(awake).toBeChecked();
		engine.respondSettings = () => {
			engine.seedAppSettings(
				{ keepAwakeWhileWorking: false },
				{ code: 'io_error', category: 'io', message: 'Disk full' },
			);
			return undefined;
		};
		fireEvent.click(awake);
		flush();

		await vi.waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('Disk full'));
		expect(awake).not.toBeChecked();
		expect(engine.settingsIntents).toContainEqual({ kind: 'setKeepAwake', enabled: false });
	});

	it('sends the category choice unchanged and shows what the engine kept', async () => {
		await renderOpenDialog((seeded) =>
			seeded.change((state) => {
				Object.assign(state.remote.connection, {
					baseUrl: '',
					categoryIds: [3030],
					apiKeyConfigured: false,
				});
			}),
		);
		expect(screen.getByTestId('app-settings-indexer-category')).toHaveTextContent(
			'Audiobooks (3030)',
		);
		fireEvent.click(screen.getByTestId('app-settings-indexer-category'));
		flush();
		const audiobooks = screen.getByRole('checkbox', { name: 'Audiobooks (3030)' });
		fireEvent.click(audiobooks);
		flush();

		await vi.waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'remote',
				intent: { kind: 'editConnection', baseUrl: null, categoryIds: [], apiKey: null },
			}),
		);
		engine.change((state) => {
			state.remote.connection.emptyCategoriesRefused = true;
		});
		expect(await screen.findByTestId('indexer-category-required')).toHaveTextContent(
			'One category is required. Audiobooks (3030) is recommended.',
		);
		expect(audiobooks).toBeChecked();
	});

	it('shows the saved Indexer URL and categories from the engine snapshot', async () => {
		await renderOpenDialog((fake) =>
			fake.change((state) => {
				Object.assign(state.remote.connection, {
					baseUrl: 'https://prowlarr.example.com',
					categoryIds: [3030],
				});
			}),
		);

		await vi.waitFor(() =>
			expect(screen.getByLabelText('URL')).toHaveValue('https://prowlarr.example.com'),
		);
		expect(screen.getByTestId('app-settings-indexer-category')).toHaveTextContent(
			'Audiobooks (3030)',
		);
	});

	it('recommends HTTPS while keeping an explicit HTTP connection usable', async () => {
		const r = await renderOpenDialog((fake) =>
			fake.change((state) => {
				Object.assign(state.remote.connection, {
					baseUrl: 'http://saved:9696',
					categoryIds: [3030],
					apiKeyConfigured: true,
				});
			}),
		);
		const url = screen.getByLabelText('URL');
		await vi.waitFor(() => expect(url).toHaveValue('http://saved:9696'));
		expect(url).toHaveAttribute('placeholder', 'https://prowlarr.example.com');
		expect(screen.getByText('HTTP is unencrypted.')).toBeInTheDocument();
		const test = vi.spyOn(r.remoteSource, 'testIndexerConnection').mockResolvedValue();
		fireEvent.click(screen.getByRole('button', { name: 'Test' }));
		flush();
		expect(test).toHaveBeenCalledOnce();
		fireEvent.input(url, { target: { value: 'https://saved.example.com' } });
		flush();
		expect(screen.queryByText('HTTP is unencrypted.')).not.toBeInTheDocument();
		expect(screen.getByText('HTTPS recommended.')).toBeInTheDocument();
	});

	it('opens connection help by click or focus and dismisses it before Settings on Escape', async () => {
		const r = await renderOpenDialog();
		const help = screen.getByRole('button', { name: 'About indexer connection security' });
		expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
		fireEvent.click(help);
		flush();
		expect(screen.getByRole('tooltip')).toHaveTextContent('Use HTTPS when available');
		fireEvent.keyDown(help, { key: 'Escape' });
		flush();
		expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
		expect(r.settings.dialog().isOpen).toBe(true);
		fireEvent.focusIn(help);
		flush();
		expect(screen.getByRole('tooltip')).toBeInTheDocument();
		fireEvent.focusOut(help, { relatedTarget: screen.getByLabelText('URL') });
		flush();
		expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
		fireEvent.keyDown(help, { key: 'Escape' });
		flush();
		expect(r.settings.dialog().isOpen).toBe(false);
	});

	it('keeps Indexer drafts when another Settings field changes', async () => {
		const r = await renderOpenDialog((fake) =>
			fake.change((state) => {
				Object.assign(state.remote.connection, {
					baseUrl: 'http://saved:9696',
					categoryIds: [3030],
					apiKeyConfigured: true,
				});
			}),
		);
		await vi.waitFor(() => expect(screen.getByLabelText('URL')).toHaveValue('http://saved:9696'));
		fireEvent.input(screen.getByLabelText('URL'), { target: { value: 'http://draft:9696' } });
		flush();
		fireEvent.input(screen.getByLabelText('API key'), { target: { value: 'draft-secret' } });
		flush();
		engine.sessionIntents.length = 0;
		fireEvent.click(screen.getByTestId('app-settings-keep-awake-checkbox'));
		flush();
		expect(engine.sessionIntents).not.toContainEqual({
			kind: 'remote',
			intent: { kind: 'loadConnection' },
		});
		expect(screen.getByLabelText('URL')).toHaveValue('http://draft:9696');
		expect(screen.getByLabelText('API key')).toHaveValue('draft-secret');
		expect(r.remoteSource.indexerConnection().apiKeyDraft).toBe('draft-secret');
	});

	it('dispatches Test for the current connection draft without saving or clearing the password', async () => {
		const r = await renderOpenDialog((fake) =>
			fake.change((state) => {
				Object.assign(state.remote.connection, {
					baseUrl: 'http://saved:9696',
					categoryIds: [3030],
					apiKeyConfigured: true,
				});
			}),
		);
		await vi.waitFor(() => expect(screen.getByLabelText('URL')).toHaveValue('http://saved:9696'));
		const test = vi.spyOn(r.remoteSource, 'testIndexerConnection').mockResolvedValue();
		const save = vi.spyOn(r.remoteSource, 'saveIndexerConnectionSettings');
		const key = screen.getByLabelText('API key');
		expect(key).toHaveAttribute('type', 'password');
		fireEvent.input(screen.getByLabelText('URL'), { target: { value: 'http://draft:9696' } });
		flush();
		fireEvent.input(key, { target: { value: 'draft-secret' } });
		flush();
		fireEvent.click(screen.getByRole('button', { name: 'Test' }));
		flush();
		expect(test).toHaveBeenCalledOnce();
		expect(save).not.toHaveBeenCalled();
		expect(r.remoteSource.indexerConnection()).toMatchObject({
			baseUrlDraft: 'http://draft:9696',
			apiKeyDraft: 'draft-secret',
		});
		expect(key).toHaveValue('draft-secret');
	});
});
