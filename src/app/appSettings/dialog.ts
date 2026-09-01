import { createSignal, type Accessor } from 'solid-js';
import { toUserMessage } from '../../lib/tauri/appError';
import type { SettingsCapability } from '../../lib/tauri/capabilities/settings';
import type { AppSettings, PinnedDefaults, StartupBehavior } from '../../types/appSettings';
import type { EncoderAvailability } from '../../types/audio';
import { setFdkAfterburner as applyFdkAfterburner } from '../../ui/encoderPanel';
import { hydrateAppSettingsProduction } from './hydrate';

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
	setFdkAfterburner(enabled: boolean): void;
	bindAfterReset(apply: ((defaults: PinnedDefaults) => void) | undefined): void;
	reset(): void;
};

export function createSettingsDialog(deps: {
	readonly capability: () => SettingsCapability;
}): SettingsDialog {
	let dialog = createInitialState();
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	let afterSettingsReset: ((defaults: PinnedDefaults) => void) | undefined;

	function update(mutator: (draft: AppSettingsDialogState) => void): void {
		const next = { ...dialog };
		mutator(next);
		dialog = next;
		bump((n) => n + 1);
	}

	async function refreshEncoderAvailability(): Promise<void> {
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

	async function reloadDialogData(): Promise<void> {
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
		await refreshEncoderAvailability();
	}

	return {
		state: () => {
			rev();
			return dialog;
		},
		async open() {
			update((draft) => {
				draft.isOpen = true;
				draft.saveState = 'idle';
				draft.saveError = '';
				draft.startupSaveState = 'idle';
				draft.startupSaveError = '';
			});
			await reloadDialogData();
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
			await refreshEncoderAvailability();
		},
		async saveCurrentSettingsAsPinnedDefaults() {
			update((draft) => {
				draft.startupSaveState = 'saving';
				draft.startupSaveError = '';
			});
			try {
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
			update((draft) => {
				draft.saveState = 'saving';
				draft.saveError = '';
			});
			try {
				await deps.capability().resetAppSettings();
				const defaults = await hydrateAppSettingsProduction();
				if (defaults) {
					afterSettingsReset?.(defaults);
				}
				update((draft) => {
					draft.saveState = 'saved';
				});
			} catch (error) {
				update((draft) => {
					draft.saveState = 'error';
					draft.saveError = describeError(error);
				});
			}
			await reloadDialogData();
		},
		setFdkAfterburner(enabled) {
			applyFdkAfterburner(enabled);
		},
		bindAfterReset(apply) {
			afterSettingsReset = apply;
		},
		reset() {
			afterSettingsReset = undefined;
			dialog = createInitialState();
			bump((n) => n + 1);
		},
	};
}
