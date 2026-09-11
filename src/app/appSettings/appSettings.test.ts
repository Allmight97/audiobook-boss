import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings } from '../../types/appSettings';
import type { SettingsCapability } from '../../lib/tauri/capabilities/settings';
import {
	encoderAvailabilityFixture,
	runtimeSettingsCapabilitiesFixture,
} from '../../test/fixtures/runtimeSettingsCapabilities';
import { createAppRuntime, type AppRuntime } from '../runtime';

function settingsFixture(overrides: Partial<AppSettings> = {}): AppSettings {
	return {
		maxConcurrentJobs: { mode: 'auto' },
		encoderDefaults: {
			settings: {
				encoderType: 'auto',
				bitrateKbps: 64,
				bitrateMode: { mode: 'vbr', value: 3 },
				channels: 'auto',
				afterburner: true,
			},
			sampleRate: 'auto',
		},
		outputDefaults: {
			outputNaming: {
				preset: 'absDefault',
				includeYear: false,
			},
		},
		toolchain: {},
		startupBehavior: 'rememberLastState',
		defaultAcquisitionLane: 'audible',
		...overrides,
	};
}

function fakeSettings(overrides: Partial<SettingsCapability> = {}): SettingsCapability {
	return {
		openFdkSetup: vi.fn(async () => undefined),
		getAppSettings: vi.fn(async () => settingsFixture()),
		getAppSettingsRecovery: vi.fn(async () => null),
		recoverAppSettings: vi.fn(async () => ({
			backupFileName: 'backup.json',
			settings: settingsFixture(),
		})),
		updateAppSettings: vi.fn(async (patch) =>
			settingsFixture({
				maxConcurrentJobs: patch.maxConcurrentJobs ?? { mode: 'auto' },
				defaultAcquisitionLane:
					patch.defaultAcquisitionLane ?? settingsFixture().defaultAcquisitionLane,
			}),
		),
		resetAppSettings: vi.fn(async () => settingsFixture()),
		openFile: vi.fn(async () => null),
		getMaxConcurrentJobs: vi.fn(async () => 4),
		setMaxConcurrentJobs: vi.fn(async (value) => value ?? 4),
		getRuntimeSettingsCapabilities: vi.fn(async () => runtimeSettingsCapabilitiesFixture()),
		...overrides,
	};
}

describe('app settings concurrency', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
		vi.restoreAllMocks();
	});

	it('hydrates auto selection and the effective backend count', async () => {
		const settings = fakeSettings();
		runtime = createAppRuntime({ settings });
		await runtime.settings.hydrateConcurrency();
		expect(runtime.settings.concurrency().effectiveLabel).toBe('Auto → 4');
		expect(runtime.settings.concurrency()).toMatchObject({
			selection: 'auto',
			effectiveLabel: 'Auto → 4',
			allowAuto: true,
			fixedOptions: [1, 2, 3, 4, 5, 6, 7, 8],
		});
		expect(settings.setMaxConcurrentJobs).toHaveBeenCalledWith(null);
	});

	it('keeps the previous concurrency when the runtime rejects a change', async () => {
		const settings = fakeSettings();
		runtime = createAppRuntime({ settings });
		await runtime.settings.hydrateConcurrency();
		vi.mocked(settings.setMaxConcurrentJobs).mockRejectedValueOnce(new Error('jobs active'));
		expect(runtime.settings.concurrency().effectiveLabel).toBe('Auto → 4');
		await runtime.settings.setConcurrencySelection('3');
		expect(runtime.settings.concurrency().selection).toBe('auto');
		expect(settings.updateAppSettings).not.toHaveBeenCalled();
		expect(runtime.settings.concurrency().errorMessage).toContain('jobs active');
		await runtime.settings.hydrateConcurrency();
		expect(runtime.settings.concurrency().errorMessage).toBe('');
	});

	it('keeps accepted concurrency after a storage failure and retries saving without reconfiguration', async () => {
		const settings = fakeSettings({
			updateAppSettings: vi.fn(async () => {
				throw new Error('Disk full');
			}),
		});
		runtime = createAppRuntime({ settings });
		await runtime.settings.hydrateConcurrency();
		await runtime.settings.setConcurrencySelection('3');
		expect(runtime.settings.concurrency()).toMatchObject({ selection: '3', effective: 3 });
		expect(runtime.settings.durability()).toMatchObject({ state: 'error', message: 'Disk full' });
		vi.mocked(settings.updateAppSettings).mockResolvedValue(
			settingsFixture({ maxConcurrentJobs: { mode: 'fixed', value: 3 } }),
		);
		await runtime.settings.retryPersistence();
		expect(runtime.settings.durability().state).toBe('saved');
		expect(settings.setMaxConcurrentJobs).toHaveBeenCalledTimes(2);
		expect(settings.updateAppSettings).toHaveBeenLastCalledWith({
			maxConcurrentJobs: { mode: 'fixed', value: 3 },
		});
	});

	it('retries the newest encoder and output defaults through the composed Settings owner', async () => {
		const settings = fakeSettings({
			updateAppSettings: vi.fn(async () => {
				throw new Error('Disk full');
			}),
		});
		runtime = createAppRuntime({ settings });
		runtime.encoding.setAfterburner(false);
		runtime.output.setAbsIncludeYear(true);
		await runtime.settings.retryPersistence();
		expect(runtime.settings.durability().state).toBe('error');
		expect(runtime.encoding.readDefaults().settings.afterburner).toBe(false);
		expect(runtime.output.readDefaults().outputNaming.includeYear).toBe(true);
		runtime.encoding.setAfterburner(true);
		vi.mocked(settings.updateAppSettings).mockResolvedValue(settingsFixture());
		await runtime.settings.retryPersistence();
		expect(settings.updateAppSettings).toHaveBeenLastCalledWith(
			expect.objectContaining({
				encoderDefaults: expect.objectContaining({
					settings: expect.objectContaining({ afterburner: true }),
				}),
				outputDefaults: expect.objectContaining({
					outputNaming: expect.objectContaining({ includeYear: true }),
				}),
			}),
		);
		expect(runtime.settings.durability().state).toBe('saved');
	});

	it('does not report an older write as durable while a newer choice is waiting', async () => {
		let finish!: (value: AppSettings) => void;
		const settings = fakeSettings({
			updateAppSettings: vi.fn(
				() =>
					new Promise<AppSettings>((resolve) => {
						finish = resolve;
					}),
			),
		});
		runtime = createAppRuntime({ settings });
		runtime.encoding.setAfterburner(false);
		await vi.waitFor(() => expect(settings.updateAppSettings).toHaveBeenCalledTimes(1));
		runtime.encoding.setAfterburner(true);
		finish(settingsFixture());
		await vi.waitFor(() => expect(settings.updateAppSettings).toHaveBeenCalledTimes(2));
		expect(runtime.settings.durability().state).toBe('saving');
		finish(settingsFixture());
		await vi.waitFor(() => expect(runtime?.settings.durability().state).toBe('saved'));
	});

	it('opens and refreshes the acquisition preference through a detached UI handler', async () => {
		const settings = fakeSettings({
			openFdkSetup: vi.fn(async () => undefined),
			getAppSettings: vi.fn(async () => settingsFixture({ defaultAcquisitionLane: 'indexer' })),
		});
		runtime = createAppRuntime({ settings });
		const openSettings = runtime.settings.openDialog;
		await expect(openSettings()).resolves.toBeUndefined();
		expect(runtime.settings.dialog().isOpen).toBe(true);
		expect(runtime.settings.defaultAcquisitionLane()).toBe('indexer');
	});

	it('preserves an unsaved acquisition choice when Settings is reopened', async () => {
		const settings = fakeSettings({
			updateAppSettings: vi.fn(async () => {
				throw new Error('Disk full');
			}),
		});
		runtime = createAppRuntime({ settings });
		await runtime.settings.setDefaultAcquisitionLane('indexer');
		await runtime.settings.openDialog();
		expect(runtime.settings.defaultAcquisitionLane()).toBe('indexer');
		expect(runtime.settings.durability().state).toBe('error');
	});

	it('retries accepted session preferences after recovery without resetting runtime choices', async () => {
		let recovered = false;
		const settings = fakeSettings({
			getAppSettings: vi.fn(async () => {
				if (!recovered) throw new Error('Unsupported saved encoder');
				return settingsFixture();
			}),
			getAppSettingsRecovery: vi.fn(async () => ({
				incompatibleEncoders: [{ scope: 'pinned' as const, encoderType: 'future_encoder' }],
			})),
			recoverAppSettings: vi.fn(async () => {
				recovered = true;
				return { backupFileName: 'backup.json', settings: settingsFixture() };
			}),
			updateAppSettings: vi.fn(async (patch) => {
				if (!recovered) throw new Error('Unsupported saved encoder');
				return settingsFixture(patch);
			}),
		});
		runtime = createAppRuntime({ settings });
		runtime.encoding.setAfterburner(false);
		await vi.waitFor(() => expect(runtime!.settings.durability().state).toBe('error'));
		await runtime.settings.openDialog();
		await runtime.settings.recoverEncoderDefaults();
		expect(runtime.encoding.readDefaults().settings.afterburner).toBe(false);
		expect(settings.updateAppSettings).toHaveBeenLastCalledWith(
			expect.objectContaining({
				encoderDefaults: expect.objectContaining({
					settings: expect.objectContaining({ afterburner: false }),
				}),
			}),
		);
		expect(settings.setMaxConcurrentJobs).not.toHaveBeenCalled();
		expect(runtime.settings.durability().state).toBe('saved');
	});

	it('does not capture stale defaults when current settings cannot be saved', async () => {
		const settings = fakeSettings({
			updateAppSettings: vi.fn(async () => {
				throw new Error('Disk full');
			}),
		});
		runtime = createAppRuntime({ settings });
		runtime.encoding.setAfterburner(false);
		await runtime.settings.saveCurrentSettingsAsPinnedDefaults();
		expect(runtime.settings.dialog().startupSaveState).toBe('error');
		expect(settings.updateAppSettings).not.toHaveBeenCalledWith(
			expect.objectContaining({ pinnedDefaults: expect.anything() }),
		);
	});

	it('orders reset after an in-flight write and drops superseded pending defaults', async () => {
		let finish!: (value: AppSettings) => void;
		const settings = fakeSettings({
			updateAppSettings: vi.fn(
				() =>
					new Promise<AppSettings>((resolve) => {
						finish = resolve;
					}),
			),
		});
		runtime = createAppRuntime({ settings });
		runtime.encoding.setAfterburner(false);
		await vi.waitFor(() => expect(settings.updateAppSettings).toHaveBeenCalledTimes(1));
		runtime.output.setAbsIncludeYear(true);
		const reset = runtime.settings.resetAllAppSettings();
		expect(settings.resetAppSettings).not.toHaveBeenCalled();
		finish(settingsFixture());
		await reset;
		expect(settings.updateAppSettings).toHaveBeenCalledTimes(1);
		expect(runtime.encoding.readDefaults().settings.afterburner).toBe(true);
		expect(runtime.output.readDefaults().outputNaming.includeYear).toBe(false);
		expect(runtime.settings.durability().state).toBe('saved');
		await runtime.settings.retryPersistence();
		expect(settings.updateAppSettings).toHaveBeenCalledTimes(1);
	});

	it('preserves defaults accepted while reset is pending, including their failed write and retry', async () => {
		let finishReset!: (value: AppSettings) => void;
		const settings = fakeSettings({
			resetAppSettings: vi.fn(
				() =>
					new Promise<AppSettings>((resolve) => {
						finishReset = resolve;
					}),
			),
			updateAppSettings: vi.fn(async () => {
				throw new Error('Disk full');
			}),
		});
		runtime = createAppRuntime({ settings });
		const reset = runtime.settings.resetAllAppSettings();
		await vi.waitFor(() => expect(settings.resetAppSettings).toHaveBeenCalled());
		runtime.encoding.setAfterburner(false);
		runtime.output.setAbsIncludeYear(true);
		const changeLane = runtime.settings.setDefaultAcquisitionLane('indexer');
		finishReset(settingsFixture());
		await Promise.all([reset, changeLane]);
		expect(runtime.encoding.readDefaults().settings.afterburner).toBe(false);
		expect(runtime.output.readDefaults().outputNaming.includeYear).toBe(true);
		expect(runtime.settings.defaultAcquisitionLane()).toBe('indexer');
		expect(runtime.settings.durability()).toMatchObject({ state: 'error', message: 'Disk full' });
		vi.mocked(settings.updateAppSettings).mockResolvedValue(settingsFixture());
		await runtime.settings.retryPersistence();
		expect(settings.updateAppSettings).toHaveBeenLastCalledWith(
			expect.objectContaining({
				encoderDefaults: expect.objectContaining({
					settings: expect.objectContaining({ afterburner: false }),
				}),
				outputDefaults: expect.objectContaining({
					outputNaming: expect.objectContaining({ includeYear: true }),
				}),
				defaultAcquisitionLane: 'indexer',
			}),
		);
		expect(runtime.settings.durability().state).toBe('saved');
	});

	it.each([true, false])(
		'keeps reset and a later concurrency request in runtime order (accepted: %s)',
		async (accepted) => {
			let finishReset!: (value: AppSettings) => void;
			let effective = 4;
			const settings = fakeSettings({
				resetAppSettings: vi.fn(async () => {
					const defaults = await new Promise<AppSettings>((resolve) => {
						finishReset = resolve;
					});
					effective = 4;
					return defaults;
				}),
				getMaxConcurrentJobs: vi.fn(async () => effective),
				setMaxConcurrentJobs: vi.fn(async (value) => {
					if (value === 3 && !accepted) throw new Error('jobs active');
					effective = value ?? 4;
					return effective;
				}),
			});
			runtime = createAppRuntime({ settings });
			await runtime.settings.setConcurrencySelection('2');
			const reset = runtime.settings.resetAllAppSettings();
			await vi.waitFor(() => expect(settings.resetAppSettings).toHaveBeenCalled());
			const change = runtime.settings.setConcurrencySelection('3');
			finishReset(settingsFixture());
			await Promise.all([reset, change]);
			expect(runtime.settings.concurrency()).toMatchObject({
				selection: accepted ? '3' : 'auto',
				effective: accepted ? 3 : 4,
				errorMessage: accepted ? '' : 'jobs active',
			});
			expect(effective).toBe(accepted ? 3 : 4);
		},
	);

	it.each([true, false])(
		'supersedes an earlier pending concurrency request only when reset succeeds (%s)',
		async (resetAccepted) => {
			let finishChange!: (value: number) => void;
			const settings = fakeSettings({
				setMaxConcurrentJobs: vi.fn(
					() =>
						new Promise<number>((resolve) => {
							finishChange = resolve;
						}),
				),
				resetAppSettings: vi.fn(async () => {
					if (!resetAccepted) throw new Error('Reset failed');
					return settingsFixture();
				}),
				updateAppSettings: vi.fn(async () => {
					throw new Error('Disk full');
				}),
			});
			runtime = createAppRuntime({ settings });
			const change = runtime.settings.setConcurrencySelection('3');
			await vi.waitFor(() => expect(settings.setMaxConcurrentJobs).toHaveBeenCalled());
			const reset = runtime.settings.resetAllAppSettings();
			finishChange(3);
			await Promise.all([change, reset]);
			expect(runtime.settings.concurrency().selection).toBe(resetAccepted ? 'auto' : '3');
			expect(runtime.settings.durability().state).toBe(resetAccepted ? 'saved' : 'error');
			vi.mocked(settings.updateAppSettings).mockResolvedValue(settingsFixture());
			await runtime.settings.retryPersistence();
			if (resetAccepted) {
				expect(settings.updateAppSettings).not.toHaveBeenCalled();
			} else {
				expect(settings.updateAppSettings).toHaveBeenLastCalledWith({
					maxConcurrentJobs: { mode: 'fixed', value: 3 },
				});
			}
		},
	);

	it('ignores a late failed write after the runtime is disposed', async () => {
		let fail!: (error: Error) => void;
		const settings = fakeSettings({
			updateAppSettings: vi.fn(
				() =>
					new Promise<AppSettings>((_, reject) => {
						fail = reject;
					}),
			),
		});
		runtime = createAppRuntime({ settings });
		runtime.encoding.setAfterburner(false);
		await vi.waitFor(() => expect(settings.updateAppSettings).toHaveBeenCalled());
		runtime.dispose();
		fail(new Error('Disk full'));
		await Promise.resolve();
		await Promise.resolve();
		expect(runtime.settings.durability().state).toBe('saved');
	});

	it('keeps pending defaults retryable if reset itself fails', async () => {
		const settings = fakeSettings({
			updateAppSettings: vi.fn(async () => {
				throw new Error('Disk full');
			}),
			resetAppSettings: vi.fn(async () => {
				throw new Error('Reset failed');
			}),
		});
		runtime = createAppRuntime({ settings });
		runtime.encoding.setAfterburner(false);
		await runtime.settings.resetAllAppSettings();
		expect(runtime.settings.durability().state).toBe('error');
		expect(runtime.encoding.readDefaults().settings.afterburner).toBe(false);
		vi.mocked(settings.updateAppSettings).mockResolvedValue(settingsFixture());
		await runtime.settings.retryPersistence();
		expect(settings.updateAppSettings).toHaveBeenLastCalledWith(
			expect.objectContaining({
				encoderDefaults: expect.objectContaining({
					settings: expect.objectContaining({ afterburner: false }),
				}),
			}),
		);
	});

	it('does not publish a pending dialog read after disposal', async () => {
		let finish!: (value: AppSettings) => void;
		const settings = fakeSettings({
			openFdkSetup: vi.fn(async () => undefined),
			getAppSettings: vi.fn(
				() =>
					new Promise<AppSettings>((resolve) => {
						finish = resolve;
					}),
			),
		});
		runtime = createAppRuntime({ settings });
		const opening = runtime.settings.openDialog();
		await vi.waitFor(() => expect(settings.getAppSettings).toHaveBeenCalled());
		runtime.dispose();
		finish(settingsFixture());
		// openDialog must not start a new preference read after its dialog was disposed.
		await opening;
		expect(runtime.settings.dialog()).toMatchObject({
			isOpen: false,
			settings: null,
			loading: false,
		});
	});

	it('hydrates and persists defaultAcquisitionLane', async () => {
		const settings = fakeSettings({
			openFdkSetup: vi.fn(async () => undefined),
			getAppSettings: vi.fn(async () => settingsFixture({ defaultAcquisitionLane: 'indexer' })),
		});
		runtime = createAppRuntime({ settings });
		await runtime.settings.hydrateAcquisitionPreferences();
		expect(runtime.settings.defaultAcquisitionLane()).toBe('indexer');

		await runtime.settings.setDefaultAcquisitionLane('audible');
		expect(settings.updateAppSettings).toHaveBeenCalledWith({ defaultAcquisitionLane: 'audible' });
		expect(runtime.settings.defaultAcquisitionLane()).toBe('audible');
		await runtime.settings.setDefaultAcquisitionLane('indexer');
		vi.mocked(settings.getAppSettings).mockResolvedValue(settingsFixture());
		await runtime.settings.resetAllAppSettings();
		expect(runtime.settings.dialog().settings?.defaultAcquisitionLane).toBe('audible');
		expect(runtime.settings.defaultAcquisitionLane()).toBe('audible');
	});

	it('refreshes Auto after accepting a custom FFmpeg path', async () => {
		let fdkAvailable = false;
		const settings = fakeSettings({
			getRuntimeSettingsCapabilities: vi.fn(async () =>
				runtimeSettingsCapabilitiesFixture({
					encoder: { availability: encoderAvailabilityFixture({ fdkAvailable }) },
				}),
			),
			updateAppSettings: vi.fn(async (patch) => {
				fdkAvailable = Boolean(patch.toolchain?.externalFfmpegPath);
				return settingsFixture({ toolchain: patch.toolchain ?? {} });
			}),
		});
		runtime = createAppRuntime({ settings });
		await runtime.settings.openDialog();
		expect(runtime.encoding.view().fdkSetupNeeded).toBe(true);
		vi.mocked(settings.getRuntimeSettingsCapabilities).mockClear();
		runtime.settings.setFfmpegPathDraft('/custom/bin/ffmpeg');
		await runtime.settings.saveToolchainPreference();
		expect(runtime.encoding.view().fdkSetupNeeded).toBe(false);
		expect(runtime.encoding.view().availabilityHint).toContain('Using external FDK AAC');
		expect(settings.getRuntimeSettingsCapabilities).toHaveBeenCalledTimes(1);
		vi.mocked(settings.getRuntimeSettingsCapabilities).mockClear();
		await runtime.settings.recheckFdk();
		expect(settings.getRuntimeSettingsCapabilities).toHaveBeenCalledTimes(1);
	});

	it('refreshes encoder availability when resetting removes the configured FFmpeg', async () => {
		let fdkAvailable = true;
		let refreshRequested = false;
		let finishRefresh!: () => void;
		const refresh = new Promise<void>((resolve) => {
			finishRefresh = resolve;
		});
		const settings = fakeSettings({
			getRuntimeSettingsCapabilities: vi.fn(async () => {
				if (!fdkAvailable) {
					refreshRequested = true;
					await refresh;
				}
				return runtimeSettingsCapabilitiesFixture({
					encoder: { availability: encoderAvailabilityFixture({ fdkAvailable }) },
				});
			}),
			resetAppSettings: vi.fn(async () => {
				fdkAvailable = false;
				return settingsFixture();
			}),
		});
		runtime = createAppRuntime({ settings });
		const encoding = runtime.encoding;
		await vi.waitFor(() => {
			expect(
				encoding.view().flavorOptions.find(({ value }) => value === 'fdk_he_aac'),
			).toMatchObject({ disabled: false });
		});
		encoding.select('encoder', 'fdk_he_aac');
		vi.mocked(settings.getRuntimeSettingsCapabilities).mockClear();
		const reset = runtime.settings.resetAllAppSettings();
		await vi.waitFor(() => expect(refreshRequested).toBe(true));
		expect(runtime.settings.dialog().saveState).toBe('saving');
		finishRefresh();
		await reset;
		expect(settings.getRuntimeSettingsCapabilities).toHaveBeenCalledTimes(1);
		expect(encoding.view().flavorOptions.find(({ value }) => value === 'fdk_he_aac')).toMatchObject(
			{ disabled: false, label: 'FDK AAC (Set up…)' },
		);
		encoding.select('encoder', 'fdk_he_aac');
		expect(encoding.request().encoderSettings.encoderType).toBe('auto');
	});
});
