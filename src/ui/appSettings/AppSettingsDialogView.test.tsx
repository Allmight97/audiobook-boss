import { flush } from 'solid-js';
import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AppSettingsRecoveryPlan } from '../../types/appSettings';
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

	/** Opens the dialog over an engine prepared by `prepare`. */
	async function renderOpenDialog(
		prepare: (engine: FakeEngine) => void = () => {},
	): Promise<AppRuntime> {
		engine = createFakeEngine();
		prepare(engine);
		runtime = createAppRuntime({ engine });
		await runtime.settings.openDialog();
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

	it('offers targeted recovery after inspection and reports its backup only after success', async () => {
		const plan: AppSettingsRecoveryPlan = {
			incompatibleEncoders: [{ scope: 'pinned', encoderType: 'future_encoder' }],
		};
		await renderOpenDialog((engine) => engine.breakSettings(plan));
		expect(screen.getByRole('listitem')).toHaveTextContent('Pinned defaults: future_encoder');
		expect(
			screen.getByText(/Output folders and other preferences will be preserved/),
		).toBeInTheDocument();
		expect(sent('recover')).toBe(0);
		await fireEvent.click(screen.getByRole('button', { name: 'Back up and recover defaults' }));
		await vi.waitFor(() =>
			expect(engine.settingsIntents).toContainEqual({ kind: 'recover', expected: plan }),
		);
		await vi.waitFor(() =>
			expect(screen.getByText(/Saved defaults recovered/)).toBeInTheDocument(),
		);
		expect(
			screen.queryByRole('button', { name: 'Back up and recover defaults' }),
		).not.toBeInTheDocument();
		expect(screen.getByText('app-settings.before-recovery.json')).toBeInTheDocument();
	});

	it('keeps the confirmed full reset reachable when targeted recovery is unavailable', async () => {
		await renderOpenDialog((engine) => engine.breakSettings());
		expect(
			screen.queryByRole('button', { name: 'Back up and recover defaults' }),
		).not.toBeInTheDocument();
		await fireEvent.click(screen.getByTestId('app-settings-reset'));
		expect(sent('reset')).toBe(0);
		await fireEvent.click(screen.getByTestId('app-settings-reset-confirm'));
		await vi.waitFor(() => expect(sent('reset')).toBe(1));
	});

	it('keeps recovery retryable when the backup cannot be saved', async () => {
		await renderOpenDialog((engine) => {
			engine.breakSettings({
				incompatibleEncoders: [{ scope: 'pinned', encoderType: 'future_encoder' }],
			});
			engine.recoverError = { message: 'Cannot write settings backup' };
		});
		await fireEvent.click(screen.getByRole('button', { name: 'Back up and recover defaults' }));
		await vi.waitFor(() =>
			expect(screen.getByText('Cannot write settings backup')).toBeInTheDocument(),
		);
		expect(screen.getByRole('button', { name: 'Back up and recover defaults' })).toBeEnabled();
		expect(screen.queryByText(/Saved defaults recovered/)).not.toBeInTheDocument();
	});

	it('requires a second activation before resetting all settings', async () => {
		await renderOpenDialog();

		await fireEvent.click(screen.getByTestId('app-settings-reset'));
		expect(sent('reset')).toBe(0);
		expect(screen.getByTestId('app-settings-reset-confirm-prompt')).toBeInTheDocument();

		await fireEvent.click(screen.getByTestId('app-settings-reset-confirm'));
		await vi.waitFor(() => expect(sent('reset')).toBe(1));
	});

	it('shows an automatic save failure and retries without discarding the accepted choice', async () => {
		await renderOpenDialog((engine) => {
			engine.settingsWriteError = { message: 'Disk full' };
		});
		engine.change((state) => {
			state.audio.defaults.choice.intent = 'encode';
		});
		flush();
		await fireEvent.change(screen.getByTestId('encoder-select'), {
			target: { value: 'faac' },
		});
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'setDefaultAudio',
				edit: { field: 'encoder', value: 'faac' },
			}),
		);
		// The engine applies the choice and records it; the write fails.
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
		engine.recordSettings({ encoderDefaults: faac });
		await vi.waitFor(() =>
			expect(screen.getByRole('button', { name: 'Retry save' })).toBeInTheDocument(),
		);
		expect(screen.getByRole('status')).toHaveTextContent(
			'Your current choices still apply for this session. Disk full',
		);
		expect(screen.getByTestId('encoder-select')).toHaveValue('faac');
		engine.settingsWriteError = undefined;
		await fireEvent.click(screen.getByRole('button', { name: 'Retry save' }));
		await vi.waitFor(() =>
			expect(screen.queryByText("Settings haven't been saved")).not.toBeInTheDocument(),
		);
		expect(screen.getByTestId('encoder-select')).toHaveValue('faac');
		expect(engine.settings().settings?.encoderDefaults.settings.encoderType).toBe('faac');
	});

	it('returns to idle when the confirm step is cancelled', async () => {
		await renderOpenDialog();

		await fireEvent.click(screen.getByTestId('app-settings-reset'));
		await fireEvent.click(screen.getByTestId('app-settings-reset-cancel'));

		expect(sent('reset')).toBe(0);
		expect(screen.queryByTestId('app-settings-reset-confirm-prompt')).not.toBeInTheDocument();
		expect(screen.getByTestId('app-settings-reset')).toBeInTheDocument();
	});

	it('returns to idle when a click lands outside the reset row', async () => {
		await renderOpenDialog();

		await fireEvent.click(screen.getByTestId('app-settings-reset'));
		await fireEvent.click(screen.getByRole('heading', { name: 'App Settings' }));

		expect(screen.queryByTestId('app-settings-reset-confirm-prompt')).not.toBeInTheDocument();
	});

	it('closes on Escape after opening post-mount, even with focus outside the dialog', async () => {
		runtime = createAppRuntime({ engine: createFakeEngine() });
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<AppSettingsDialogView />
			</AppRuntimeProvider>
		));
		expect(runtime.settings.dialog().isOpen).toBe(false);

		await runtime.settings.openDialog();
		await fireEvent.keyDown(document.body, { key: 'Escape' });

		expect(runtime.settings.dialog().isOpen).toBe(false);
	});

	it('records the default acquisition lane when the Indexer radio is selected', async () => {
		await renderOpenDialog();

		await fireEvent.click(screen.getByTestId('app-settings-default-lane-indexer'));

		await vi.waitFor(() => expect(runtime!.settings.defaultAcquisitionLane()).toBe('indexer'));
		expect(engine.settingsIntents).toContainEqual({
			kind: 'remember',
			defaultAcquisitionLane: 'indexer',
		});
	});

	it('shows the awake preference the engine holds and keeps the old choice when a change is refused', async () => {
		await renderOpenDialog();

		let awake = screen.getByTestId('app-settings-keep-awake-checkbox');
		expect(awake).toBeChecked();
		await fireEvent.click(awake);
		await vi.waitFor(() => {
			expect(awake).not.toBeChecked();
			expect(awake).toBeEnabled();
		});
		expect(engine.settingsIntents).toContainEqual({ kind: 'setKeepAwake', enabled: false });

		runtime!.settings.closeDialog();
		await runtime!.settings.openDialog();
		awake = screen.getByTestId('app-settings-keep-awake-checkbox');
		expect(awake).not.toBeChecked();

		engine.settingsWriteError = { message: 'Settings file is read-only' };
		await fireEvent.click(awake);
		await vi.waitFor(() =>
			expect(screen.getByTestId('app-settings-power-error')).toHaveTextContent(
				'Settings file is read-only',
			),
		);
		expect(awake).not.toBeChecked();
	});

	it('lets the user enable both audiobook categories from the collapsed picker', async () => {
		await renderOpenDialog((engine) =>
			engine.change((state) => {
				Object.assign(state.remote.connection, {
					baseUrl: '',
					categoryIds: [3030],
					apiKeyConfigured: false,
				});
			}),
		);
		await vi.waitFor(() =>
			expect(runtime!.remoteSource.indexerConnection().categoryIdsDraft).toEqual([3030]),
		);

		expect(screen.getByTestId('app-settings-indexer-category')).toHaveTextContent(
			'Audiobooks (3030)',
		);
		await fireEvent.click(screen.getByTestId('app-settings-indexer-category'));
		const audio = screen.getByRole('checkbox', { name: 'Audio (3000)' }) as HTMLInputElement;
		audio.checked = true;
		audio.dispatchEvent(new Event('change', { bubbles: true }));
		flush();

		expect(runtime!.remoteSource.indexerConnection().categoryIdsDraft).toEqual([3030, 3000]);
		expect(screen.getByTestId('app-settings-indexer-category')).toHaveTextContent(
			'Audiobooks (3030), Audio (3000)',
		);
	});

	it('recommends HTTPS while keeping an explicit HTTP connection usable', async () => {
		const r = await renderOpenDialog((engine) =>
			engine.change((state) => {
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
		await fireEvent.click(screen.getByRole('button', { name: 'Test' }));
		expect(test).toHaveBeenCalledOnce();
		await fireEvent.input(url, { target: { value: 'https://saved.example.com' } });
		expect(screen.queryByText('HTTP is unencrypted.')).not.toBeInTheDocument();
		expect(screen.getByText('HTTPS recommended.')).toBeInTheDocument();
	});

	it('opens connection help by click or focus and dismisses it before Settings on Escape', async () => {
		const r = await renderOpenDialog();
		const help = screen.getByRole('button', { name: 'About indexer connection security' });
		expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
		await fireEvent.click(help);
		expect(screen.getByRole('tooltip')).toHaveTextContent('Use HTTPS when available');
		await fireEvent.keyDown(help, { key: 'Escape' });
		expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
		expect(r.settings.dialog().isOpen).toBe(true);
		await fireEvent.focusIn(help);
		expect(screen.getByRole('tooltip')).toBeInTheDocument();
		await fireEvent.focusOut(help, { relatedTarget: screen.getByLabelText('URL') });
		expect(screen.queryByRole('tooltip')).not.toBeInTheDocument();
		await fireEvent.keyDown(help, { key: 'Escape' });
		expect(r.settings.dialog().isOpen).toBe(false);
	});

	it('keeps Indexer drafts when another Settings field changes', async () => {
		const r = await renderOpenDialog((engine) =>
			engine.change((state) => {
				Object.assign(state.remote.connection, {
					baseUrl: 'http://saved:9696',
					categoryIds: [3030],
					apiKeyConfigured: true,
				});
			}),
		);
		await vi.waitFor(() => expect(screen.getByLabelText('URL')).toHaveValue('http://saved:9696'));
		await fireEvent.input(screen.getByLabelText('URL'), { target: { value: 'http://draft:9696' } });
		await fireEvent.input(screen.getByLabelText('API key'), { target: { value: 'draft-secret' } });
		engine.sessionIntents.length = 0;
		await fireEvent.click(screen.getByTestId('app-settings-keep-awake-checkbox'));
		expect(engine.sessionIntents).not.toContainEqual({
			kind: 'remote',
			intent: { kind: 'loadConnection' },
		});
		expect(screen.getByLabelText('URL')).toHaveValue('http://draft:9696');
		expect(screen.getByLabelText('API key')).toHaveValue('draft-secret');
		expect(r.remoteSource.indexerConnection().apiKeyDraft).toBe('draft-secret');
	});

	it('dispatches Test for the current connection draft without saving or clearing the password', async () => {
		const r = await renderOpenDialog((engine) =>
			engine.change((state) => {
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
		await fireEvent.input(screen.getByLabelText('URL'), { target: { value: 'http://draft:9696' } });
		await fireEvent.input(key, { target: { value: 'draft-secret' } });
		await fireEvent.click(screen.getByRole('button', { name: 'Test' }));
		expect(test).toHaveBeenCalledOnce();
		expect(save).not.toHaveBeenCalled();
		expect(r.remoteSource.indexerConnection()).toMatchObject({
			baseUrlDraft: 'http://draft:9696',
			apiKeyDraft: 'draft-secret',
		});
		expect(key).toHaveValue('draft-secret');
	});
});
