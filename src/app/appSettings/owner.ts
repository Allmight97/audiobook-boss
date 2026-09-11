import { createSignal, type Accessor } from 'solid-js';
import type {
	AcquisitionLane,
	AppSettings,
	EncoderDefaults,
	OutputDefaults,
	ConcurrencyPreference,
	PinnedDefaults,
	StartupBehavior,
} from '../../types/appSettings';
import type { EncoderSettingsCapabilities, MaxConcurrentJobsCapabilities } from '../../types/audio';
import {
	liveSettingsCapability,
	type SettingsCapability,
} from '../../lib/tauri/capabilities/settings';
import { toUserMessage } from '../../lib/tauri/appError';
import { createSettingsDialog, type AppSettingsDialogState } from './dialog';
import { resolveStartupDefaults } from './startupDefaults';

export type SettingsDurability = {
	readonly state: 'saved' | 'saving' | 'error';
	readonly message: string;
};

export type ConcurrencyView = {
	readonly errorMessage: string;
	readonly selection: string;
	readonly effective: number | null;
	readonly effectiveLabel: string;
	readonly controlsEnabled: boolean;
	readonly allowAuto: boolean;
	readonly fixedOptions: ReadonlyArray<number>;
};

export type SettingsOwner = {
	readonly durability: Accessor<SettingsDurability>;
	rememberEncoderDefaults(defaults: EncoderDefaults): void;
	rememberOutputDefaults(defaults: OutputDefaults): void;
	retryPersistence(): Promise<void>;
	readonly concurrency: Accessor<ConcurrencyView>;
	readonly defaultAcquisitionLane: Accessor<AcquisitionLane>;
	readonly capability: Accessor<SettingsCapability>;
	readonly dialog: Accessor<AppSettingsDialogState>;
	hydrateConcurrency(input?: {
		readonly preference?: ConcurrencyPreference;
		readonly capabilities?: MaxConcurrentJobsCapabilities | null;
	}): Promise<void>;
	hydrateAcquisitionPreferences(): Promise<void>;
	setConcurrencySelection(value: string): Promise<void>;
	setDefaultAcquisitionLane(lane: AcquisitionLane): Promise<void>;
	setControlsEnabled(enabled: boolean): void;
	openDialog(): Promise<void>;
	closeDialog(): void;
	setDialogOpen(open: boolean): void;
	browseForFfmpegBinary(): Promise<void>;
	clearFfmpegPathDraft(): void;
	setFfmpegPathDraft(value: string): void;
	saveToolchainPreference(): Promise<void>;
	recheckFdk(): Promise<void>;
	openFdkSetup(): Promise<void>;
	saveCurrentSettingsAsPinnedDefaults(): Promise<void>;
	setStartupBehavior(behavior: StartupBehavior): Promise<void>;
	resetAllAppSettings(): Promise<void>;
	recoverEncoderDefaults(): Promise<void>;
	bindAfterReset(apply: ((defaults: PinnedDefaults) => void | Promise<void>) | undefined): void;
	reset(): void;
};

export type SettingsOwnerDeps = {
	readonly capability?: SettingsCapability;
	readonly onToolchainChanged?: (capabilities: EncoderSettingsCapabilities | null) => Promise<void>;
};

type RememberedDefaults = Partial<
	Pick<
		AppSettings,
		'encoderDefaults' | 'outputDefaults' | 'maxConcurrentJobs' | 'defaultAcquisitionLane'
	>
>;

function emptyConcurrency(): ConcurrencyView {
	return {
		errorMessage: '',
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
	let concurrency = emptyConcurrency();
	let defaultAcquisitionLane: AcquisitionLane = 'audible';
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	const capabilityValue = deps.capability ?? liveSettingsCapability;
	const capability: Accessor<SettingsCapability> = () => capabilityValue;
	let generation = 0;
	let concurrencyRevision = 0;
	let laneRevision = 0;
	let writeRevision = 0;
	let pendingPatch: RememberedDefaults = {};
	let durability: SettingsDurability = { state: 'saved', message: '' };
	let writeQueue: Promise<void> = Promise.resolve();
	let afterSettingsReset: ((defaults: PinnedDefaults) => void | Promise<void>) | undefined;

	function enqueue<T>(action: () => Promise<T>): Promise<T> {
		const next = writeQueue.then(action);
		writeQueue = next.then(
			() => undefined,
			() => undefined,
		);
		return next;
	}

	function publishDurability(next: SettingsDurability): void {
		durability = next;
		bump((n) => n + 1);
	}

	async function persistPending(): Promise<void> {
		if (Object.keys(pendingPatch).length === 0) return;
		const revision = ++writeRevision;
		const started = generation;
		publishDurability({ state: 'saving', message: durability.message });
		await enqueue(async () => {
			if (
				started !== generation ||
				revision !== writeRevision ||
				Object.keys(pendingPatch).length === 0
			)
				return;
			try {
				await capabilityValue.updateAppSettings(pendingPatch);
				if (started !== generation || revision !== writeRevision) return;
				pendingPatch = {};
				publishDurability({ state: 'saved', message: '' });
			} catch (error) {
				if (started !== generation || revision !== writeRevision) return;
				publishDurability({ state: 'error', message: toUserMessage(error) });
			}
		});
	}

	function remember(patch: RememberedDefaults): Promise<void> {
		pendingPatch = { ...pendingPatch, ...patch };
		return persistPending();
	}

	const dialogCapability: SettingsCapability = {
		...capabilityValue,
		getAppSettings: () => enqueue(() => capabilityValue.getAppSettings()),
		updateAppSettings: (patch) => enqueue(() => capabilityValue.updateAppSettings(patch)),
		getAppSettingsRecovery: () => enqueue(() => capabilityValue.getAppSettingsRecovery()),
		recoverAppSettings: async (expected) => {
			const started = generation;
			const result = await enqueue(() => capabilityValue.recoverAppSettings(expected));
			if (started !== generation) return result;
			if (Object.keys(pendingPatch).length === 0)
				publishDurability({ state: 'saved', message: '' });
			else await persistPending();
			return result;
		},
		resetAppSettings: () => {
			writeRevision += 1;
			concurrencyRevision += 1;
			const started = generation;
			const supersededPatch = pendingPatch;
			pendingPatch = {};
			return enqueue(async () => {
				// Earlier concurrency requests finish before reset reaches the runtime.
				if (pendingPatch.maxConcurrentJobs) {
					supersededPatch.maxConcurrentJobs = pendingPatch.maxConcurrentJobs;
					delete pendingPatch.maxConcurrentJobs;
				}
				let settings: AppSettings;
				try {
					settings = await capabilityValue.resetAppSettings();
				} catch (error) {
					if (started === generation) {
						pendingPatch = { ...supersededPatch, ...pendingPatch };
						if (Object.keys(pendingPatch).length > 0)
							publishDurability({ state: 'error', message: toUserMessage(error) });
					}
					throw error;
				}
				if (started !== generation) return settings;
				if (Object.keys(pendingPatch).length === 0)
					publishDurability({ state: 'saved', message: '' });
				await reflectResetConcurrency(settings.maxConcurrentJobs, started);
				if (started !== generation) return settings;
				const accepted = { ...settings, ...pendingPatch };
				commitDefaultLane(accepted.defaultAcquisitionLane ?? 'audible');
				await afterSettingsReset?.(accepted);
				return accepted;
			});
		},
	};
	const dialog = createSettingsDialog({
		capability: () => dialogCapability,
		onToolchainChanged: deps.onToolchainChanged,
		beforeCapture: async () => {
			await persistPending();
			if (durability.state === 'error')
				throw new Error(`Save current settings before pinning defaults. ${durability.message}`);
		},
	});

	function commitConcurrency(next: ConcurrencyView): void {
		concurrency = next;
		bump((n) => n + 1);
	}

	function commitDefaultLane(lane: AcquisitionLane): void {
		defaultAcquisitionLane = lane;
		bump((n) => n + 1);
	}

	async function reflectResetConcurrency(
		preference: ConcurrencyPreference,
		started: number,
	): Promise<void> {
		const selection = preference.mode === 'fixed' ? String(preference.value) : 'auto';
		let effective: number | null = null;
		let errorMessage = '';
		try {
			effective = await capabilityValue.getMaxConcurrentJobs();
		} catch (error) {
			errorMessage = toUserMessage(error);
		}
		if (started !== generation) return;
		commitConcurrency({
			...concurrency,
			selection,
			effective,
			errorMessage,
			effectiveLabel: labelFor(selection, effective),
		});
	}

	async function hydrateAcquisitionPreferences() {
		if (laneRevision > 0) return;
		const started = generation;
		const revision = laneRevision;
		try {
			const settings = await capabilityValue.getAppSettings();
			if (started !== generation || revision !== laneRevision) return;
			commitDefaultLane(settings.defaultAcquisitionLane ?? 'audible');
		} catch (error) {
			console.warn('Failed to hydrate acquisition preferences:', error);
		}
	}

	return {
		durability: () => {
			rev();
			return durability;
		},
		rememberEncoderDefaults: (defaults) => {
			void remember({ encoderDefaults: defaults });
		},
		rememberOutputDefaults: (defaults) => {
			void remember({ outputDefaults: defaults });
		},
		retryPersistence: persistPending,
		concurrency: () => {
			rev();
			return concurrency;
		},
		defaultAcquisitionLane: () => {
			rev();
			return defaultAcquisitionLane;
		},
		capability,
		dialog: dialog.state,
		async hydrateConcurrency(input = {}) {
			const started = generation;
			const revision = concurrencyRevision;
			try {
				const runtime = await capabilityValue.getRuntimeSettingsCapabilities();
				const source = await resolveStartupDefaults(capabilityValue);
				const capabilities = input.capabilities ?? runtime.maxConcurrentJobs ?? null;
				const preference = input.preference ?? source.maxConcurrentJobs;
				const selection = preference.mode === 'fixed' ? String(preference.value) : 'auto';
				const effective = await enqueue(async () => {
					if (started !== generation || revision !== concurrencyRevision) return null;
					return capabilityValue.setMaxConcurrentJobs(
						preference.mode === 'auto' ? null : preference.value,
					);
				});
				if (started !== generation || revision !== concurrencyRevision || effective === null)
					return;
				const latest = concurrency;
				commitConcurrency({
					...latest,
					errorMessage: '',
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
		hydrateAcquisitionPreferences,
		async setConcurrencySelection(value) {
			const started = generation;
			concurrencyRevision += 1;
			const preference = preferenceFromSelection(value);
			try {
				const effective = await enqueue(async () => {
					if (started !== generation) return null;
					return capabilityValue.setMaxConcurrentJobs(
						preference.mode === 'auto' ? null : preference.value,
					);
				});
				if (started !== generation || effective === null) return;
				const accepted: ConcurrencyPreference =
					preference.mode === 'auto' ? preference : { mode: 'fixed', value: effective };
				const selection = accepted.mode === 'fixed' ? String(accepted.value) : 'auto';
				commitConcurrency({
					...concurrency,
					selection,
					effective,
					errorMessage: '',
					effectiveLabel: labelFor(selection, effective),
				});
				await remember({ maxConcurrentJobs: accepted });
			} catch (error) {
				if (started !== generation) return;
				commitConcurrency({ ...concurrency, errorMessage: toUserMessage(error) });
			}
		},
		async setDefaultAcquisitionLane(lane) {
			laneRevision += 1;
			commitDefaultLane(lane);
			await remember({ defaultAcquisitionLane: lane });
		},
		setControlsEnabled(enabled) {
			commitConcurrency({ ...concurrency, controlsEnabled: enabled });
		},
		async openDialog() {
			const started = generation;
			await dialog.open();
			if (started !== generation) return;
			await hydrateAcquisitionPreferences();
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
		recheckFdk: () => dialog.recheckFdk(),
		openFdkSetup: () => dialog.openFdkSetup(),
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
		recoverEncoderDefaults() {
			return dialog.recoverEncoderDefaults();
		},
		bindAfterReset(apply) {
			afterSettingsReset = apply;
		},
		reset() {
			generation += 1;
			writeRevision += 1;
			pendingPatch = {};
			afterSettingsReset = undefined;
			publishDurability({ state: 'saved', message: '' });
			dialog.reset();
			commitConcurrency(emptyConcurrency());
			commitDefaultLane('audible');
		},
	};
}
