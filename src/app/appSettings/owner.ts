import { createSignal, type Accessor } from 'solid-js';
import type {
	ConcurrencyPreference,
	PinnedDefaults,
	StartupBehavior,
} from '../../types/appSettings';
import type { MaxConcurrentJobsCapabilities } from '../../types/audio';
import {
	liveSettingsCapability,
	type SettingsCapability,
} from '../../lib/tauri/capabilities/settings';
import { createSettingsDialog, type AppSettingsDialogState } from './dialog';
import { resolveStartupDefaults } from './startupDefaults';

export type ConcurrencyView = {
	readonly selection: string;
	readonly effective: number | null;
	readonly effectiveLabel: string;
	readonly controlsEnabled: boolean;
	readonly allowAuto: boolean;
	readonly fixedOptions: ReadonlyArray<number>;
};

export type SettingsOwner = {
	readonly concurrency: Accessor<ConcurrencyView>;
	readonly capability: Accessor<SettingsCapability>;
	readonly dialog: Accessor<AppSettingsDialogState>;
	hydrateConcurrency(input?: {
		readonly preference?: ConcurrencyPreference;
		readonly capabilities?: MaxConcurrentJobsCapabilities | null;
	}): Promise<void>;
	setConcurrencySelection(value: string): Promise<void>;
	setControlsEnabled(enabled: boolean): void;
	openDialog(): Promise<void>;
	closeDialog(): void;
	setDialogOpen(open: boolean): void;
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

export type SettingsOwnerDeps = {
	readonly capability?: SettingsCapability;
};

function emptyConcurrency(): ConcurrencyView {
	return {
		selection: 'auto',
		effective: null,
		effectiveLabel: '',
		controlsEnabled: true,
		allowAuto: true,
		fixedOptions: [],
	};
}

function labelFor(selection: string, effective: number | null): string {
	if (effective === null) return '';
	return selection === 'auto' ? `Auto → ${effective}` : `Max ${effective}`;
}

function preferenceFromSelection(value: string): ConcurrencyPreference {
	if (value === 'auto') return { mode: 'auto' };
	const parsed = Number.parseInt(value, 10);
	return { mode: 'fixed', value: parsed };
}

export function createSettingsOwner(deps: SettingsOwnerDeps = {}): SettingsOwner {
	const [concurrency, setConcurrency] = createSignal(emptyConcurrency());
	const [capability] = createSignal(deps.capability ?? liveSettingsCapability);
	const dialog = createSettingsDialog({ capability: () => capability() });

	return {
		concurrency,
		capability,
		dialog: dialog.state,
		async hydrateConcurrency(input = {}) {
			try {
				const runtime = await capability().getRuntimeSettingsCapabilities();
				const source = await resolveStartupDefaults(capability());
				const capabilities = input.capabilities ?? runtime.maxConcurrentJobs ?? null;
				const preference = input.preference ?? source.maxConcurrentJobs;
				const selection = preference.mode === 'fixed' ? String(preference.value) : 'auto';
				const effective =
					selection === 'auto'
						? await capability().setMaxConcurrentJobs(null)
						: await capability().setMaxConcurrentJobs(Number.parseInt(selection, 10));
				const latest = concurrency();
				setConcurrency({
					...latest,
					selection,
					effective,
					allowAuto: capabilities?.allowAuto ?? true,
					fixedOptions: capabilities?.fixedOptions ?? [],
					effectiveLabel: labelFor(selection, effective),
				});
			} catch (error) {
				console.warn('Failed to hydrate max concurrency:', error);
			}
		},
		async setConcurrencySelection(value) {
			const previous = concurrency().selection;
			setConcurrency({ ...concurrency(), selection: value });
			try {
				const preference = preferenceFromSelection(value);
				const settings = await capability().updateAppSettings({ maxConcurrentJobs: preference });
				const accepted = settings.maxConcurrentJobs;
				const selection = accepted.mode === 'fixed' ? String(accepted.value) : 'auto';
				let effective: number | null = null;
				try {
					effective = await capability().getMaxConcurrentJobs();
				} catch {
					effective = accepted.mode === 'fixed' ? accepted.value : null;
				}
				setConcurrency({
					...concurrency(),
					selection,
					effective,
					effectiveLabel: labelFor(selection, effective),
				});
			} catch (error) {
				console.warn('Failed to update max concurrency:', error);
				setConcurrency({ ...concurrency(), selection: previous });
			}
		},
		setControlsEnabled(enabled) {
			setConcurrency({ ...concurrency(), controlsEnabled: enabled });
		},
		openDialog() {
			return dialog.open();
		},
		closeDialog() {
			dialog.close();
		},
		setDialogOpen(open) {
			dialog.setOpen(open);
		},
		browseForFfmpegBinary() {
			return dialog.browseForFfmpegBinary();
		},
		clearFfmpegPathDraft() {
			dialog.clearFfmpegPathDraft();
		},
		setFfmpegPathDraft(value) {
			dialog.setFfmpegPathDraft(value);
		},
		saveToolchainPreference() {
			return dialog.saveToolchainPreference();
		},
		saveCurrentSettingsAsPinnedDefaults() {
			return dialog.saveCurrentSettingsAsPinnedDefaults();
		},
		setStartupBehavior(behavior) {
			return dialog.setStartupBehavior(behavior);
		},
		resetAllAppSettings() {
			return dialog.resetAllAppSettings();
		},
		setFdkAfterburner(enabled) {
			dialog.setFdkAfterburner(enabled);
		},
		bindAfterReset(apply) {
			dialog.bindAfterReset(apply);
		},
		reset() {
			dialog.reset();
			setConcurrency(emptyConcurrency());
		},
	};
}
