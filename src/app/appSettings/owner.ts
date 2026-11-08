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
} from '../../lib/generated/tauri';
import { concurrencyLabel } from '../../lib/format/concurrency';
import { logFrontendFailure } from '../../lib/frontendLogBridge';
import { QUIT_SHORTCUT, runsOnMac } from '../../lib/platform';
import { toUserMessage } from '../../lib/tauri/appError';
import type { EngineLink } from '../engineLink';

type SettingsDurability = {
	readonly state: 'saved' | 'saving' | 'error';
	readonly message: string;
};

type ConcurrencyView = {
	readonly errorMessage: string;
	readonly selection: string;
	readonly autoSelection: string;
	readonly effective: number;
	readonly autoEffective: number;
	readonly label: string;
	readonly locked: boolean;
	readonly fixedOptions: ReadonlyArray<number>;
};

type SettingsSaveState = 'idle' | 'saving' | 'saved' | 'error';

type SettingStatus = { readonly state: SettingsSaveState; readonly error: string };

type DialogSetting = 'reset' | 'keepAwake' | 'pressQuitTwice' | 'aacDecoder' | 'startup' | 'lane';

const RETRY_DROPPED = "Retry didn't reach ABB.";

const IDLE: SettingStatus = { state: 'idle', error: '' };

type AppSettingsDialogState = {
	isOpen: boolean;
	settings: AppSettings;
	/** Only macOS has the Quit shortcut the Quitting setting governs. */
	quittingAvailable: boolean;
	status: Readonly<Record<DialogSetting, SettingStatus>>;
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
	openDialog: () => void;
	closeDialog: () => void;
	setKeepAwakeWhileWorking: (enabled: boolean) => Promise<void>;
	setPressQuitTwice: (enabled: boolean) => Promise<void>;
	setAacDecoder: (decoder: AacDecoder) => Promise<void>;
	saveCurrentSettingsAsPinnedDefaults: () => Promise<void>;
	setStartupBehavior: (behavior: StartupBehavior) => Promise<void>;
	resetAllAppSettings: () => Promise<void>;
};

export type SettingsOwnerDeps = {
	readonly link: EngineLink;
};

type DialogProgress = Omit<AppSettingsDialogState, 'settings' | 'quittingAvailable'>;

function idleDialog(): DialogProgress {
	return {
		isOpen: false,
		status: {
			reset: IDLE,
			keepAwake: IDLE,
			pressQuitTwice: IDLE,
			aacDecoder: IDLE,
			startup: IDLE,
			lane: IDLE,
		},
	};
}

function describe(error: unknown): string {
	return toUserMessage(error, { fallback: 'Settings update failed.' });
}

const UNIT_LABELS: Record<SettingsUnit, string> = {
	keepAwake: 'keep computer awake while working',
	pressQuitTwice: `press ${QUIT_SHORTCUT} twice to quit`,
	jobLimit: 'number of jobs',
	acquisitionLane: 'default import source',
	aacDecoder: 'AAC decoder',
	indexerConnection: 'Indexer URL and categories',
	encoderDefaults: 'audio defaults',
	outputDefaults: 'output folder and naming',
	startupBehavior: 'startup settings',
	pinnedDefaults: 'pinned defaults',
};

const AUTO_SELECTION = 'auto';

function selectionOf(preference: ConcurrencyPreference): string {
	return preference.mode === 'fixed' ? String(preference.value) : AUTO_SELECTION;
}

function preferenceFromSelection(value: string): ConcurrencyPreference {
	return value === AUTO_SELECTION
		? { mode: 'auto' }
		: { mode: 'fixed', value: Number.parseInt(value, 10) };
}

export function createSettingsOwner(deps: SettingsOwnerDeps): SettingsOwner {
	const { link } = deps;
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	let dialog = idleDialog();
	let concurrencyError = '';
	let writesInFlight = 0;
	let retryDropped = false;
	let latestConcurrencyRequest = 0;
	const latestRequest: Record<DialogSetting, number> = {
		reset: 0,
		keepAwake: 0,
		pressQuitTwice: 0,
		aacDecoder: 0,
		startup: 0,
		lane: 0,
	};

	function changed(): void {
		bump((n) => n + 1);
	}

	function updateDialog(patch: Partial<DialogProgress>): void {
		dialog = { ...dialog, ...patch };
		changed();
	}

	async function send(intent: SettingsIntent): Promise<SettingsOutcome | { error: unknown }> {
		try {
			const outcome = await link.sendSettings(intent);
			retryDropped = false;
			return outcome;
		} catch (error) {
			return { error };
		}
	}

	function failure(outcome: SettingsOutcome | { error: unknown }): string | null {
		return 'error' in outcome ? describe(outcome.error) : null;
	}

	function setStatus(setting: DialogSetting, status: SettingStatus): void {
		updateDialog({ status: { ...dialog.status, [setting]: status } });
	}

	async function runDialogAction(setting: DialogSetting, intent: SettingsIntent): Promise<boolean> {
		const request = ++latestRequest[setting];
		setStatus(setting, { state: 'saving', error: '' });
		const error = failure(await send(intent));
		if (request === latestRequest[setting]) {
			setStatus(
				setting,
				error === null ? { state: 'saved', error: '' } : { state: 'error', error },
			);
		}
		return error === null;
	}

	return {
		durability: () => {
			rev();
			const error = link.settings().saveError;
			const reason = error ? toUserMessage(error) : '';
			if (writesInFlight > 0) return { state: 'saving', message: reason };
			if (!error) return { state: 'saved', message: '' };
			return {
				state: 'error',
				message: retryDropped ? `${RETRY_DROPPED} ${reason}` : reason,
			};
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
			retryDropped = false;
			changed();
			const outcome = await send({ kind: 'retry' });
			writesInFlight -= 1;
			if ('error' in outcome) {
				logFrontendFailure('appSettings.retry', outcome.error);
				retryDropped = true;
			}
			changed();
		},
		concurrency: () => {
			rev();
			const snapshot = link.settings();
			const { preference, effective, capabilities } = snapshot.concurrency;
			return {
				errorMessage: concurrencyError,
				selection: selectionOf(preference),
				autoSelection: AUTO_SELECTION,
				effective,
				autoEffective: capabilities.autoEffective,
				label: concurrencyLabel(snapshot.concurrency),
				locked: link.concurrencyLocked(),
				fixedOptions: capabilities.fixedOptions,
			};
		},
		defaultAcquisitionLane: () => link.settings().settings.defaultAcquisitionLane,
		dialog: () => {
			rev();
			return { ...dialog, settings: link.settings().settings, quittingAvailable: runsOnMac() };
		},
		async setConcurrencySelection(value) {
			const request = ++latestConcurrencyRequest;
			const error = failure(
				await send({ kind: 'setConcurrency', preference: preferenceFromSelection(value) }),
			);
			if (request !== latestConcurrencyRequest) return;
			concurrencyError = error ?? '';
			changed();
		},
		async setDefaultAcquisitionLane(defaultAcquisitionLane) {
			await runDialogAction('lane', {
				kind: 'setDefaultAcquisitionLane',
				lane: defaultAcquisitionLane,
			});
		},
		openDialog() {
			updateDialog({ ...idleDialog(), isOpen: true });
		},
		closeDialog() {
			updateDialog({ isOpen: false });
		},
		async setKeepAwakeWhileWorking(enabled) {
			await runDialogAction('keepAwake', { kind: 'setKeepAwake', enabled });
		},
		async setPressQuitTwice(enabled) {
			await runDialogAction('pressQuitTwice', { kind: 'setPressQuitTwice', enabled });
		},
		async setAacDecoder(decoder) {
			await runDialogAction('aacDecoder', { kind: 'setAacDecoder', decoder });
		},
		async saveCurrentSettingsAsPinnedDefaults() {
			await runDialogAction('startup', { kind: 'pinCurrentDefaults' });
		},
		async setStartupBehavior(behavior) {
			await runDialogAction('startup', { kind: 'setStartupBehavior', behavior });
		},
		async resetAllAppSettings() {
			if (!(await runDialogAction('reset', { kind: 'reset' }))) return;
			concurrencyError = '';
			changed();
		},
	};
}
