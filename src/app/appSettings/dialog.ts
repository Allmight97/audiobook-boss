import { toUserMessage } from '../../lib/tauri/appError';
import { liveSettingsCapability } from '../../lib/tauri/capabilities/settings';
import type { AppSettings, PinnedDefaults, StartupBehavior } from '../../types/appSettings';
import type { EncoderAvailability } from '../../types/audio';
import { setFdkAfterburner } from '../../ui/encoderPanel';
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

export const productionSettingsDialogState: AppSettingsDialogState = createInitialState();

let afterSettingsReset: ((defaults: PinnedDefaults) => void) | undefined;

export function bindAfterSettingsReset(
	apply: ((defaults: PinnedDefaults) => void) | undefined,
): void {
	afterSettingsReset = apply;
}

const listeners = new Set<() => void>();

export function subscribeProductionSettingsDialog(listener: () => void): () => void {
	listeners.add(listener);
	return () => {
		listeners.delete(listener);
	};
}

function notify(): void {
	for (const listener of listeners) listener();
}

function describeError(error: unknown): string {
	return toUserMessage(error, { fallback: 'Settings update failed.' });
}

async function refreshEncoderAvailability(): Promise<void> {
	try {
		const capabilities = await liveSettingsCapability.getRuntimeSettingsCapabilities();
		productionSettingsDialogState.encoderAvailability = capabilities.encoder?.availability ?? null;
	} catch {
		productionSettingsDialogState.encoderAvailability = null;
	}
	notify();
}

async function reloadDialogData(): Promise<void> {
	productionSettingsDialogState.loading = true;
	notify();
	try {
		const settings = await liveSettingsCapability.getAppSettings();
		productionSettingsDialogState.settings = settings;
		productionSettingsDialogState.ffmpegPathDraft = settings.toolchain?.externalFfmpegPath ?? '';
	} catch (error) {
		productionSettingsDialogState.settings = null;
		productionSettingsDialogState.saveState = 'error';
		productionSettingsDialogState.saveError = describeError(error);
	} finally {
		productionSettingsDialogState.loading = false;
		notify();
	}
	await refreshEncoderAvailability();
}

export async function openProductionSettingsDialog(): Promise<void> {
	productionSettingsDialogState.isOpen = true;
	productionSettingsDialogState.saveState = 'idle';
	productionSettingsDialogState.saveError = '';
	productionSettingsDialogState.startupSaveState = 'idle';
	productionSettingsDialogState.startupSaveError = '';
	notify();
	await reloadDialogData();
}

export function closeProductionSettingsDialog(): void {
	productionSettingsDialogState.isOpen = false;
	notify();
}

export function setProductionSettingsDialogOpen(open: boolean): void {
	productionSettingsDialogState.isOpen = open;
	notify();
}

export function resetProductionSettingsDialog(): void {
	Object.assign(productionSettingsDialogState, createInitialState());
	notify();
}

export async function browseForFfmpegBinary(): Promise<void> {
	const selected = await liveSettingsCapability.openFile({
		title: 'Choose an FFmpeg binary with libfdk_aac',
	});
	if (selected) {
		productionSettingsDialogState.ffmpegPathDraft = selected;
		notify();
	}
}

export function clearFfmpegPathDraft(): void {
	productionSettingsDialogState.ffmpegPathDraft = '';
	notify();
}

export function setFfmpegPathDraft(value: string): void {
	productionSettingsDialogState.ffmpegPathDraft = value;
	notify();
}

export async function saveToolchainPreference(): Promise<void> {
	productionSettingsDialogState.saveState = 'saving';
	productionSettingsDialogState.saveError = '';
	notify();
	const draft = productionSettingsDialogState.ffmpegPathDraft.trim();
	try {
		const settings = await liveSettingsCapability.updateAppSettings({
			toolchain: { externalFfmpegPath: draft.length > 0 ? draft : undefined },
		});
		productionSettingsDialogState.settings = settings;
		productionSettingsDialogState.ffmpegPathDraft = settings.toolchain?.externalFfmpegPath ?? '';
		productionSettingsDialogState.saveState = 'saved';
	} catch (error) {
		productionSettingsDialogState.saveState = 'error';
		productionSettingsDialogState.saveError = describeError(error);
	}
	await refreshEncoderAvailability();
	notify();
}

export async function saveCurrentSettingsAsPinnedDefaults(): Promise<void> {
	productionSettingsDialogState.startupSaveState = 'saving';
	productionSettingsDialogState.startupSaveError = '';
	notify();
	try {
		const current = await liveSettingsCapability.getAppSettings();
		const settings = await liveSettingsCapability.updateAppSettings({
			pinnedDefaults: {
				maxConcurrentJobs: current.maxConcurrentJobs,
				encoderDefaults: current.encoderDefaults,
				outputDefaults: current.outputDefaults,
			},
		});
		productionSettingsDialogState.settings = settings;
		productionSettingsDialogState.startupSaveState = 'saved';
	} catch (error) {
		productionSettingsDialogState.startupSaveState = 'error';
		productionSettingsDialogState.startupSaveError = describeError(error);
	}
	notify();
}

export async function setStartupBehavior(behavior: StartupBehavior): Promise<void> {
	productionSettingsDialogState.startupSaveState = 'saving';
	productionSettingsDialogState.startupSaveError = '';
	notify();
	try {
		const settings = await liveSettingsCapability.updateAppSettings({ startupBehavior: behavior });
		productionSettingsDialogState.settings = settings;
		productionSettingsDialogState.startupSaveState = 'saved';
	} catch (error) {
		productionSettingsDialogState.startupSaveState = 'error';
		productionSettingsDialogState.startupSaveError = describeError(error);
	}
	notify();
}

export async function resetAllAppSettings(): Promise<void> {
	productionSettingsDialogState.saveState = 'saving';
	productionSettingsDialogState.saveError = '';
	notify();
	try {
		await liveSettingsCapability.resetAppSettings();
		const defaults = await hydrateAppSettingsProduction();
		if (defaults) {
			afterSettingsReset?.(defaults);
		}
		productionSettingsDialogState.saveState = 'saved';
	} catch (error) {
		productionSettingsDialogState.saveState = 'error';
		productionSettingsDialogState.saveError = describeError(error);
	}
	await reloadDialogData();
}

export function setProductionFdkAfterburner(enabled: boolean): void {
	setFdkAfterburner(enabled);
	notify();
}
