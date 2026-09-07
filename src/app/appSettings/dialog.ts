import { createSignal, type Accessor } from 'solid-js';
import { toUserMessage } from '../../lib/tauri/appError';
import type { SettingsCapability } from '../../lib/tauri/capabilities/settings';
import type { AppSettings, PinnedDefaults, StartupBehavior } from '../../types/appSettings';
import type { EncoderAvailability } from '../../types/audio';

export type SettingsSaveState = 'idle' | 'saving' | 'saved' | 'error';

export type AppSettingsDialogState = {
	isOpen: boolean;
	loading: boolean;
	settings: AppSettings | null;
	ffmpegPathDraft: string;
	saveState: SettingsSaveState;
	saveError: string;
	encoderAvailability: EncoderAvailability | null;
	startupSaveState: SettingsSaveState;
	startupSaveError: string;
};

function createInitialState(): AppSettingsDialogState {
	return {
		isOpen: false,
		loading: false,
		settings: null,
		ffmpegPathDraft: '',
		saveState: 'idle',
		saveError: '',
		encoderAvailability: null,
		startupSaveState: 'idle',
		startupSaveError: '',
	};
}

function describeError(error: unknown): string {
	return toUserMessage(error, { fallback: 'Settings update failed.' });
}

export type SettingsDialog = {
	readonly state: Accessor<AppSettingsDialogState>;
	open(): Promise<void>;
	close(): void;
	setOpen(open: boolean): void;
	browseForFfmpegBinary(): Promise<void>;
	clearFfmpegPathDraft(): void;
	setFfmpegPathDraft(value: string): void;
	saveToolchainPreference(): Promise<void>;
	saveCurrentSettingsAsPinnedDefaults(): Promise<void>;
	setStartupBehavior(behavior: StartupBehavior): Promise<void>;
	resetAllAppSettings(): Promise<void>;
	bindAfterReset(apply: ((defaults: PinnedDefaults) => void | Promise<void>) | undefined): void;
	reset(): void;
};

export function createSettingsDialog(deps: {
	readonly capability: () => SettingsCapability;
	readonly beforeCapture: () => Promise<void>;
}): SettingsDialog {
	let dialog = createInitialState();
	let generation = 0;
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	let afterSettingsReset: ((defaults: PinnedDefaults) => void | Promise<void>) | undefined;

	function update(mutator: (draft: AppSettingsDialogState) => void): void {
		const next = { ...dialog };
		mutator(next);
		dialog = next;
		bump((n) => n + 1);
	}

	function guardedUpdate(started: number): typeof update {
		return (mutator) => {
			if (started === generation) update(mutator);
		};
	}

	async function refreshEncoderAvailability(started = generation): Promise<void> {
		if (started !== generation) return;
		const update = guardedUpdate(started);
		try {
			const capabilities = await deps.capability().getRuntimeSettingsCapabilities();
			update((draft) => {
				draft.encoderAvailability = capabilities.encoder?.availability ?? null;
			});
		} catch {
			update((draft) => {
				draft.encoderAvailability = null;
			});
		}
	}

	async function reloadDialogData(started = generation): Promise<void> {
		if (started !== generation) return;
		const update = guardedUpdate(started);
		update((draft) => {
			draft.loading = true;
		});
		try {
			const settings = await deps.capability().getAppSettings();
			update((draft) => {
				draft.settings = settings;
				draft.ffmpegPathDraft = settings.toolchain?.externalFfmpegPath ?? '';
			});
		} catch (error) {
			update((draft) => {
				draft.settings = null;
				draft.saveState = 'error';
				draft.saveError = describeError(error);
			});
		} finally {
			update((draft) => {
				draft.loading = false;
			});
		}
		await refreshEncoderAvailability(started);
	}

	return {
		state: () => {
			rev();
			return dialog;
		},
		async open() {
			const started = generation;
			const update = guardedUpdate(started);
			update((draft) => {
				draft.isOpen = true;
				draft.saveState = 'idle';
				draft.saveError = '';
				draft.startupSaveState = 'idle';
				draft.startupSaveError = '';
			});
			await reloadDialogData(started);
		},
		close() {
			update((draft) => {
				draft.isOpen = false;
			});
		},
		setOpen(open) {
			update((draft) => {
				draft.isOpen = open;
			});
		},
		async browseForFfmpegBinary() {
			const started = generation;
			const update = guardedUpdate(started);
			const selected = await deps.capability().openFile({
				title: 'Choose an FFmpeg binary with libfdk_aac',
			});
			if (selected) {
				update((draft) => {
					draft.ffmpegPathDraft = selected;
				});
			}
		},
		clearFfmpegPathDraft() {
			update((draft) => {
				draft.ffmpegPathDraft = '';
			});
		},
		setFfmpegPathDraft(value) {
			update((draft) => {
				draft.ffmpegPathDraft = value;
			});
		},
		async saveToolchainPreference() {
			const started = generation;
			const update = guardedUpdate(started);
			update((draft) => {
				draft.saveState = 'saving';
				draft.saveError = '';
			});
			const draftPath = dialog.ffmpegPathDraft.trim();
			try {
				const settings = await deps.capability().updateAppSettings({
					toolchain: { externalFfmpegPath: draftPath.length > 0 ? draftPath : undefined },
				});
				update((draft) => {
					draft.settings = settings;
					draft.ffmpegPathDraft = settings.toolchain?.externalFfmpegPath ?? '';
					draft.saveState = 'saved';
				});
			} catch (error) {
				update((draft) => {
					draft.saveState = 'error';
					draft.saveError = describeError(error);
				});
			}
			await refreshEncoderAvailability(started);
		},
		async saveCurrentSettingsAsPinnedDefaults() {
			const started = generation;
			const update = guardedUpdate(started);
			update((draft) => {
				draft.startupSaveState = 'saving';
				draft.startupSaveError = '';
			});
			try {
				await deps.beforeCapture();
				const current = await deps.capability().getAppSettings();
				const settings = await deps.capability().updateAppSettings({
					pinnedDefaults: {
						maxConcurrentJobs: current.maxConcurrentJobs,
						encoderDefaults: current.encoderDefaults,
						outputDefaults: current.outputDefaults,
					},
				});
				update((draft) => {
					draft.settings = settings;
					draft.startupSaveState = 'saved';
				});
			} catch (error) {
				update((draft) => {
					draft.startupSaveState = 'error';
					draft.startupSaveError = describeError(error);
				});
			}
		},
		async setStartupBehavior(behavior) {
			const started = generation;
			const update = guardedUpdate(started);
			update((draft) => {
				draft.startupSaveState = 'saving';
				draft.startupSaveError = '';
			});
			try {
				const settings = await deps.capability().updateAppSettings({ startupBehavior: behavior });
				update((draft) => {
					draft.settings = settings;
					draft.startupSaveState = 'saved';
				});
			} catch (error) {
				update((draft) => {
					draft.startupSaveState = 'error';
					draft.startupSaveError = describeError(error);
				});
			}
		},
		async resetAllAppSettings() {
			const started = generation;
			const update = guardedUpdate(started);
			update((draft) => {
				draft.saveState = 'saving';
				draft.saveError = '';
			});
			try {
				const defaults = await deps.capability().resetAppSettings();
				if (started !== generation) return;
				await afterSettingsReset?.(defaults);
				update((draft) => {
					draft.saveState = 'saved';
				});
			} catch (error) {
				update((draft) => {
					draft.saveState = 'error';
					draft.saveError = describeError(error);
				});
			}
			await reloadDialogData(started);
		},
		bindAfterReset(apply) {
			afterSettingsReset = apply;
		},
		reset() {
			generation += 1;
			afterSettingsReset = undefined;
			dialog = createInitialState();
			bump((n) => n + 1);
		},
	};
}
