import { createSignal, type Accessor } from 'solid-js';
import { toUserMessage } from '../../lib/tauri/appError';
import type { SettingsCapability } from '../../lib/tauri/capabilities/settings';
import type {
	AppSettings,
	AppSettingsRecoveryPlan,
	StartupBehavior,
} from '../../types/appSettings';

export type SettingsSaveState = 'idle' | 'saving' | 'saved' | 'error';

export type AppSettingsDialogState = {
	isOpen: boolean;
	loading: boolean;
	settings: AppSettings | null;
	saveState: SettingsSaveState;
	saveError: string;
	powerSaveState: SettingsSaveState;
	powerSaveError: string;
	startupSaveState: SettingsSaveState;
	startupSaveError: string;
	recovery: AppSettingsRecoveryPlan | null;
	recoveryBackup: string;
};

function createInitialState(): AppSettingsDialogState {
	return {
		isOpen: false,
		loading: false,
		settings: null,
		saveState: 'idle',
		saveError: '',
		powerSaveState: 'idle',
		powerSaveError: '',
		startupSaveState: 'idle',
		startupSaveError: '',
		recovery: null,
		recoveryBackup: '',
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
	setKeepAwakeWhileWorking(enabled: boolean): Promise<void>;
	saveCurrentSettingsAsPinnedDefaults(): Promise<void>;
	setStartupBehavior(behavior: StartupBehavior): Promise<void>;
	resetAllAppSettings(): Promise<void>;
	recoverEncoderDefaults(): Promise<void>;
	reset(): void;
};

export function createSettingsDialog(deps: {
	readonly capability: () => SettingsCapability;
	readonly beforeCapture: () => Promise<void>;
}): SettingsDialog {
	let dialog = createInitialState();
	let generation = 0;
	const [rev, bump] = createSignal(0, { ownedWrite: true });

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
				draft.recovery = null;
			});
		} catch (error) {
			update((draft) => {
				draft.settings = null;
				draft.saveState = 'error';
				draft.saveError = describeError(error);
			});
			try {
				const recovery = await deps.capability().getAppSettingsRecovery();
				update((draft) => {
					draft.recovery = recovery;
					if (recovery) draft.saveError = '';
				});
			} catch (recoveryError) {
				update((draft) => {
					draft.recovery = null;
					draft.saveError += ` Recovery check failed: ${describeError(recoveryError)}`;
				});
			}
		} finally {
			update((draft) => {
				draft.loading = false;
			});
		}
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
				draft.powerSaveState = 'idle';
				draft.powerSaveError = '';
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
		async setKeepAwakeWhileWorking(enabled) {
			const started = generation;
			const update = guardedUpdate(started);
			if (dialog.powerSaveState === 'saving') return;
			update((draft) => {
				draft.powerSaveState = 'saving';
				draft.powerSaveError = '';
			});
			try {
				const settings = await deps
					.capability()
					.updateAppSettings({ keepAwakeWhileWorking: enabled });
				update((draft) => {
					draft.settings = settings;
					draft.powerSaveState = 'saved';
				});
			} catch (error) {
				update((draft) => {
					draft.powerSaveState = 'error';
					draft.powerSaveError = describeError(error);
				});
			}
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
				await deps.capability().resetAppSettings();
				if (started !== generation) return;
				await reloadDialogData(started);
				update((draft) => {
					draft.saveState = 'saved';
				});
			} catch (error) {
				update((draft) => {
					draft.saveState = 'error';
					draft.saveError = describeError(error);
				});
			}
		},
		async recoverEncoderDefaults() {
			if (!dialog.recovery || dialog.saveState === 'saving') return;
			const expected = dialog.recovery;
			const started = generation;
			const update = guardedUpdate(started);
			update((draft) => {
				draft.saveState = 'saving';
				draft.saveError = '';
			});
			try {
				const result = await deps.capability().recoverAppSettings(expected);
				if (started !== generation) return;
				update((draft) => {
					draft.recoveryBackup = result.backupFileName;
				});
				await reloadDialogData(started);
				update((draft) => {
					if (draft.settings) draft.saveState = 'saved';
				});
			} catch (error) {
				update((draft) => {
					draft.saveState = 'error';
					draft.saveError = describeError(error);
				});
			}
		},
		reset() {
			generation += 1;
			dialog = createInitialState();
			bump((n) => n + 1);
		},
	};
}
