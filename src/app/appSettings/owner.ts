import { createSignal, type Accessor } from 'solid-js';
import type {
	AcquisitionLane,
	AppSettings,
	AppSettingsRecoveryPlan,
	ConcurrencyPreference,
	EncoderDefaults,
	OutputDefaults,
	PinnedDefaults,
	SettingsIntent,
	SettingsOutcome,
	StartupBehavior,
} from '../../types/appSettings';
import {
	liveSettingsCapability,
	type SettingsCapability,
} from '../../lib/tauri/capabilities/settings';
import { toUserMessage } from '../../lib/tauri/appError';
import type { EngineLink } from '../engineLink';

export type SettingsDurability = {
	readonly state: 'saved' | 'saving' | 'error';
	readonly message: string;
};

export type ConcurrencyView = {
	readonly errorMessage: string;
	readonly selection: string;
	readonly effective: number | null;
	readonly autoEffective: number | null;
	readonly effectiveLabel: string;
	readonly controlsEnabled: boolean;
	readonly allowAuto: boolean;
	readonly fixedOptions: ReadonlyArray<number>;
};

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

/**
 * The engine owns the settings in effect, their validation, and whether they
 * are saved. This owner shows the engine's snapshot, sends intents, and
 * keeps the dialog's per-control progress.
 */
export type SettingsOwner = {
	readonly durability: Accessor<SettingsDurability>;
	/** The defaults this launch starts from. Rejects while settings are unreadable. */
	loadStartupDefaults(): Promise<PinnedDefaults>;
	rememberEncoderDefaults(defaults: EncoderDefaults): void;
	rememberOutputDefaults(defaults: OutputDefaults): void;
	retryPersistence(): Promise<void>;
	readonly concurrency: Accessor<ConcurrencyView>;
	readonly defaultAcquisitionLane: Accessor<AcquisitionLane>;
	readonly capability: Accessor<SettingsCapability>;
	readonly dialog: Accessor<AppSettingsDialogState>;
	setConcurrencySelection(value: string): Promise<void>;
	setDefaultAcquisitionLane(lane: AcquisitionLane): Promise<void>;
	setControlsEnabled(enabled: boolean): void;
	openDialog(): Promise<void>;
	closeDialog(): void;
	setDialogOpen(open: boolean): void;
	setKeepAwakeWhileWorking(enabled: boolean): Promise<void>;
	saveCurrentSettingsAsPinnedDefaults(): Promise<void>;
	setStartupBehavior(behavior: StartupBehavior): Promise<void>;
	resetAllAppSettings(): Promise<void>;
	recoverEncoderDefaults(): Promise<void>;
	bindAfterReset(apply: ((defaults: PinnedDefaults) => void | Promise<void>) | undefined): void;
	reset(): void;
};

export type SettingsOwnerDeps = {
	readonly link: EngineLink;
	readonly capability?: SettingsCapability;
};

/** The dialog's own progress: which control is saving and how it ended. */
type DialogProgress = Omit<AppSettingsDialogState, 'settings' | 'recovery'>;
type ProgressKey = 'save' | 'powerSave' | 'startupSave';

function idleDialog(): DialogProgress {
	return {
		isOpen: false,
		loading: false,
		saveState: 'idle',
		saveError: '',
		powerSaveState: 'idle',
		powerSaveError: '',
		startupSaveState: 'idle',
		startupSaveError: '',
		recoveryBackup: '',
	};
}

function describe(error: unknown): string {
	return toUserMessage(error, { fallback: 'Settings update failed.' });
}

function selectionOf(preference: ConcurrencyPreference): string {
	return preference.mode === 'fixed' ? String(preference.value) : 'auto';
}

function preferenceFromSelection(value: string): ConcurrencyPreference {
	return value === 'auto' ? { mode: 'auto' } : { mode: 'fixed', value: Number.parseInt(value, 10) };
}

export function createSettingsOwner(deps: SettingsOwnerDeps): SettingsOwner {
	const { link } = deps;
	const capabilityValue = deps.capability ?? liveSettingsCapability;
	const capability: Accessor<SettingsCapability> = () => capabilityValue;
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	let dialog = idleDialog();
	let concurrencyError = '';
	let controlsEnabled = true;
	let writesInFlight = 0;
	// Advances on reset; a reply from before it changes nothing here.
	let generation = 0;
	let afterSettingsReset: ((defaults: PinnedDefaults) => void | Promise<void>) | undefined;

	function changed(): void {
		bump((n) => n + 1);
	}

	function updateDialog(started: number, patch: Partial<DialogProgress>): void {
		if (started !== generation) return;
		dialog = { ...dialog, ...patch };
		changed();
	}

	/** Sends an intent; a transport failure reads as a rejection. */
	async function send(intent: SettingsIntent): Promise<SettingsOutcome | { error: unknown }> {
		try {
			return await link.sendSettings(intent);
		} catch (error) {
			return { error };
		}
	}

	/** Why an intent did not apply: the engine refused it, or it never arrived. */
	function failure(outcome: SettingsOutcome | { error: unknown }): string | null {
		return 'error' in outcome ? describe(outcome.error) : null;
	}

	/** Records defaults a panel accepted. A failed write shows through `durability`. */
	async function remember(intent: Extract<SettingsIntent, { kind: 'remember' }>): Promise<void> {
		writesInFlight += 1;
		changed();
		const outcome = await send(intent);
		writesInFlight -= 1;
		changed();
		if ('error' in outcome) console.error('Failed to record settings:', outcome.error);
	}

	/** Runs a dialog action and shows its progress on the control that started it. */
	async function runDialogAction(key: ProgressKey, intent: SettingsIntent): Promise<boolean> {
		const started = generation;
		updateDialog(started, { [`${key}State`]: 'saving', [`${key}Error`]: '' });
		const error = failure(await send(intent));
		updateDialog(
			started,
			error === null
				? { [`${key}State`]: 'saved' }
				: { [`${key}State`]: 'error', [`${key}Error`]: error },
		);
		return error === null;
	}

	return {
		durability: () => {
			rev();
			const error = link.settings().saveError;
			if (writesInFlight > 0)
				return { state: 'saving', message: error ? toUserMessage(error) : '' };
			return error
				? { state: 'error', message: toUserMessage(error) }
				: { state: 'saved', message: '' };
		},
		async loadStartupDefaults() {
			await link.ready();
			const settings = link.settings();
			if (!settings.startupDefaults)
				throw settings.loadError ?? new Error('App settings are unavailable.');
			return settings.startupDefaults;
		},
		rememberEncoderDefaults: (encoderDefaults) => {
			void remember({ kind: 'remember', encoderDefaults });
		},
		rememberOutputDefaults: (outputDefaults) => {
			void remember({ kind: 'remember', outputDefaults });
		},
		async retryPersistence() {
			writesInFlight += 1;
			changed();
			await send({ kind: 'retry' });
			writesInFlight -= 1;
			changed();
		},
		concurrency: () => {
			rev();
			const snapshot = link.settings();
			const { preference, effective, capabilities } = snapshot.concurrency;
			// Before the engine has answered there is nothing truthful to show.
			const known = snapshot.revision >= 0;
			const selection = selectionOf(preference);
			return {
				errorMessage: concurrencyError,
				selection,
				effective: known ? effective : null,
				autoEffective: known ? capabilities.autoEffective : null,
				effectiveLabel: !known
					? ''
					: selection === 'auto'
						? `Auto → ${effective}`
						: `Max ${effective}`,
				controlsEnabled,
				allowAuto: capabilities.allowAuto,
				fixedOptions: known ? capabilities.fixedOptions : [],
			};
		},
		defaultAcquisitionLane: () => link.settings().defaultAcquisitionLane,
		capability,
		dialog: () => {
			rev();
			const snapshot = link.settings();
			const unreadable = !snapshot.settings && snapshot.loadError;
			return {
				...dialog,
				settings: snapshot.settings ?? null,
				recovery: snapshot.recovery ?? null,
				// Unreadable settings are the dialog's error unless a recovery is offered.
				saveState: unreadable && dialog.saveState === 'idle' ? 'error' : dialog.saveState,
				saveError:
					unreadable && !snapshot.recovery && !dialog.saveError
						? describe(snapshot.loadError)
						: dialog.saveError,
			};
		},
		async setConcurrencySelection(value) {
			const started = generation;
			const error = failure(
				await send({ kind: 'setConcurrency', preference: preferenceFromSelection(value) }),
			);
			if (started !== generation) return;
			concurrencyError = error ?? '';
			changed();
		},
		setDefaultAcquisitionLane: (defaultAcquisitionLane) =>
			remember({ kind: 'remember', defaultAcquisitionLane }),
		setControlsEnabled(enabled) {
			controlsEnabled = enabled;
			changed();
		},
		async openDialog() {
			const started = generation;
			updateDialog(started, { ...idleDialog(), isOpen: true, loading: true });
			// Settings that failed to load are read again each time the dialog opens.
			await send({ kind: 'reload' });
			updateDialog(started, { loading: false });
		},
		closeDialog() {
			updateDialog(generation, { isOpen: false });
		},
		setDialogOpen(open) {
			updateDialog(generation, { isOpen: open });
		},
		async setKeepAwakeWhileWorking(enabled) {
			if (dialog.powerSaveState === 'saving') return;
			await runDialogAction('powerSave', { kind: 'setKeepAwake', enabled });
		},
		async saveCurrentSettingsAsPinnedDefaults() {
			await runDialogAction('startupSave', { kind: 'pinCurrentDefaults' });
		},
		async setStartupBehavior(behavior) {
			await runDialogAction('startupSave', { kind: 'setStartupBehavior', behavior });
		},
		async resetAllAppSettings() {
			const started = generation;
			if (!(await runDialogAction('save', { kind: 'reset' })) || started !== generation) return;
			concurrencyError = '';
			changed();
			const defaults = link.settings().startupDefaults;
			if (defaults) await afterSettingsReset?.(defaults);
		},
		async recoverEncoderDefaults() {
			const expected = link.settings().recovery;
			if (!expected || dialog.saveState === 'saving') return;
			const started = generation;
			updateDialog(started, { saveState: 'saving', saveError: '' });
			const outcome = await send({ kind: 'recover', expected });
			const error = failure(outcome);
			if (error !== null) {
				updateDialog(started, { saveState: 'error', saveError: error });
				return;
			}
			updateDialog(started, {
				saveState: 'saved',
				recoveryBackup:
					'kind' in outcome && outcome.kind === 'recovered' ? outcome.backupFileName : '',
			});
		},
		bindAfterReset(apply) {
			afterSettingsReset = apply;
		},
		reset() {
			generation += 1;
			afterSettingsReset = undefined;
			dialog = idleDialog();
			concurrencyError = '';
			controlsEnabled = true;
			changed();
		},
	};
}
