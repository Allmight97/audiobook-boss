import { createSignal, type Accessor } from 'solid-js';
import type {
	AacDecoder,
	AcquisitionLane,
	AppSettings,
	ConcurrencyPreference,
	SettingsIntent,
	SettingsOutcome,
	SettingsUnit,
	StartupBehavior,
} from '../../types/appSettings';
import { toUserMessage } from '../../lib/tauri/appError';
import type { EngineLink } from '../engineLink';

export type SettingsDurability = {
	readonly state: 'saved' | 'saving' | 'error';
	readonly message: string;
};

export type ConcurrencyView = {
	readonly errorMessage: string;
	readonly selection: string;
	readonly effective: number;
	readonly autoEffective: number;
	readonly effectiveLabel: string;
	readonly controlsEnabled: boolean;
	readonly allowAuto: boolean;
	readonly fixedOptions: ReadonlyArray<number>;
};

export type SettingsSaveState = 'idle' | 'saving' | 'saved' | 'error';

export type AppSettingsDialogState = {
	isOpen: boolean;
	settings: AppSettings | null;
	saveState: SettingsSaveState;
	saveError: string;
	powerSaveState: SettingsSaveState;
	powerSaveError: string;
	decoderSaveState: SettingsSaveState;
	decoderSaveError: string;
	startupSaveState: SettingsSaveState;
	startupSaveError: string;
	laneSaveState: SettingsSaveState;
	laneSaveError: string;
};

export type SettingsOwner = {
	readonly durability: Accessor<SettingsDurability>;
	/** What this launch could not keep from the saved settings; empty when nothing. */
	readonly recoveryNotice: Accessor<string>;
	retryPersistence: () => Promise<void>;
	readonly concurrency: Accessor<ConcurrencyView>;
	readonly defaultAcquisitionLane: Accessor<AcquisitionLane>;
	readonly dialog: Accessor<AppSettingsDialogState>;
	setConcurrencySelection: (value: string) => Promise<void>;
	setDefaultAcquisitionLane: (lane: AcquisitionLane) => Promise<void>;
	setControlsEnabled: (enabled: boolean) => void;
	openDialog: () => void;
	closeDialog: () => void;
	setDialogOpen: (open: boolean) => void;
	setKeepAwakeWhileWorking: (enabled: boolean) => Promise<void>;
	setAacDecoder: (decoder: AacDecoder) => Promise<void>;
	saveCurrentSettingsAsPinnedDefaults: () => Promise<void>;
	setStartupBehavior: (behavior: StartupBehavior) => Promise<void>;
	resetAllAppSettings: () => Promise<void>;
	reset: () => void;
};

export type SettingsOwnerDeps = {
	readonly link: EngineLink;
	/** Called once the engine has applied a reset of every setting. */
	readonly onAppliedReset: () => void;
};

type DialogProgress = Omit<AppSettingsDialogState, 'settings'>;
type ProgressKey = 'save' | 'powerSave' | 'decoderSave' | 'startupSave' | 'laneSave';

function idleDialog(): DialogProgress {
	return {
		isOpen: false,
		saveState: 'idle',
		saveError: '',
		powerSaveState: 'idle',
		powerSaveError: '',
		decoderSaveState: 'idle',
		decoderSaveError: '',
		startupSaveState: 'idle',
		startupSaveError: '',
		laneSaveState: 'idle',
		laneSaveError: '',
	};
}

function describe(error: unknown): string {
	return toUserMessage(error, { fallback: 'Settings update failed.' });
}

const UNIT_LABELS: Record<SettingsUnit, string> = {
	keepAwake: 'keep computer awake while working',
	jobLimit: 'number of jobs',
	acquisitionLane: 'default import source',
	aacDecoder: 'AAC decoder',
	indexerConnection: 'Indexer URL and categories',
	encoderDefaults: 'audio defaults',
	outputDefaults: 'output folder and naming',
	startupBehavior: 'startup settings',
	pinnedDefaults: 'pinned defaults',
};

function selectionOf(preference: ConcurrencyPreference): string {
	return preference.mode === 'fixed' ? String(preference.value) : 'auto';
}

function preferenceFromSelection(value: string): ConcurrencyPreference {
	return value === 'auto' ? { mode: 'auto' } : { mode: 'fixed', value: Number.parseInt(value, 10) };
}

export function createSettingsOwner(deps: SettingsOwnerDeps): SettingsOwner {
	const { link } = deps;
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	let dialog = idleDialog();
	let concurrencyError = '';
	let controlsEnabled = true;
	let writesInFlight = 0;
	let resetGeneration = 0;

	function changed(): void {
		bump((n) => n + 1);
	}

	function updateDialog(started: number, patch: Partial<DialogProgress>): void {
		if (started !== resetGeneration) return;
		dialog = { ...dialog, ...patch };
		changed();
	}

	async function send(intent: SettingsIntent): Promise<SettingsOutcome | { error: unknown }> {
		try {
			return await link.sendSettings(intent);
		} catch (error) {
			return { error };
		}
	}

	function failure(outcome: SettingsOutcome | { error: unknown }): string | null {
		return 'error' in outcome ? describe(outcome.error) : null;
	}

	async function runDialogAction(key: ProgressKey, intent: SettingsIntent): Promise<boolean> {
		const started = resetGeneration;
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
		recoveryNotice: () => {
			const recovery = link.settings().recovery;
			if (!recovery) return '';
			const parts = [];
			if (recovery.reset.length > 0)
				parts.push(
					`ABB no longer accepts these saved settings, so they are back to their defaults: ${recovery.reset.map((unit) => UNIT_LABELS[unit]).join(', ')}.`,
				);
			if (recovery.setAsideAs)
				parts.push(
					`ABB couldn't read your settings file, so every setting is back to its default. The file was kept as ${recovery.setAsideAs} in ABB's settings folder.`,
				);
			if (recovery.reset.includes('indexerConnection') || recovery.setAsideAs)
				parts.push('A saved Indexer API key stays stored; enter the same URL to use it.');
			return parts.join(' ');
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
			const selection = selectionOf(preference);
			return {
				errorMessage: concurrencyError,
				selection,
				effective,
				autoEffective: capabilities.autoEffective,
				effectiveLabel: selection === 'auto' ? `Auto → ${effective}` : `Max ${effective}`,
				controlsEnabled,
				allowAuto: capabilities.allowAuto,
				fixedOptions: capabilities.fixedOptions,
			};
		},
		defaultAcquisitionLane: () => link.settings().settings?.defaultAcquisitionLane ?? 'audible',
		dialog: () => {
			rev();
			return { ...dialog, settings: link.settings().settings ?? null };
		},
		async setConcurrencySelection(value) {
			const started = resetGeneration;
			const error = failure(
				await send({ kind: 'setConcurrency', preference: preferenceFromSelection(value) }),
			);
			if (started !== resetGeneration) return;
			concurrencyError = error ?? '';
			changed();
		},
		async setDefaultAcquisitionLane(defaultAcquisitionLane) {
			await runDialogAction('laneSave', { kind: 'remember', defaultAcquisitionLane });
		},
		setControlsEnabled(enabled) {
			controlsEnabled = enabled;
			changed();
		},
		openDialog() {
			updateDialog(resetGeneration, { ...idleDialog(), isOpen: true });
		},
		closeDialog() {
			updateDialog(resetGeneration, { isOpen: false });
		},
		setDialogOpen(open) {
			updateDialog(resetGeneration, { isOpen: open });
		},
		async setKeepAwakeWhileWorking(enabled) {
			if (dialog.powerSaveState === 'saving') return;
			await runDialogAction('powerSave', { kind: 'setKeepAwake', enabled });
		},
		async setAacDecoder(decoder) {
			if (dialog.decoderSaveState === 'saving') return;
			await runDialogAction('decoderSave', { kind: 'setAacDecoder', decoder });
		},
		async saveCurrentSettingsAsPinnedDefaults() {
			await runDialogAction('startupSave', { kind: 'pinCurrentDefaults' });
		},
		async setStartupBehavior(behavior) {
			await runDialogAction('startupSave', { kind: 'setStartupBehavior', behavior });
		},
		async resetAllAppSettings() {
			const started = resetGeneration;
			if (!(await runDialogAction('save', { kind: 'reset' })) || started !== resetGeneration)
				return;
			concurrencyError = '';
			changed();
			deps.onAppliedReset();
		},
		reset() {
			resetGeneration += 1;
			dialog = idleDialog();
			concurrencyError = '';
			controlsEnabled = true;
			changed();
		},
	};
}
