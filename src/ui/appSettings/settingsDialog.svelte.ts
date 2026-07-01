import { toUserMessage } from '../../lib/tauri/appError';
import { tauriClient } from '../../lib/tauri/client';
import type { AppSettings } from '../../types/appSettings';
import type { EncoderAvailability } from '../../types/audio';
import { refreshRuntimeSettingsCapabilities } from '../runtimeSettingsCapabilities.svelte';
import { hydrateAppSettings } from './hydration';

export type SettingsSaveState = 'idle' | 'saving' | 'saved' | 'error';

type AppSettingsDialogState = {
	isOpen: boolean;
	loading: boolean;
	settings: AppSettings | null;
	ffmpegPathDraft: string;
	saveState: SettingsSaveState;
	saveError: string;
	encoderAvailability: EncoderAvailability | null;
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
	};
}

export const appSettingsDialogState = $state<AppSettingsDialogState>(createInitialState());

export async function openAppSettingsDialog(): Promise<void> {
	appSettingsDialogState.isOpen = true;
	appSettingsDialogState.saveState = 'idle';
	appSettingsDialogState.saveError = '';
	await reloadDialogData();
}

export function closeAppSettingsDialog(): void {
	appSettingsDialogState.isOpen = false;
}

async function reloadDialogData(): Promise<void> {
	appSettingsDialogState.loading = true;
	try {
		const settings = await tauriClient.getAppSettings();
		appSettingsDialogState.settings = settings;
		appSettingsDialogState.ffmpegPathDraft = settings.toolchain?.externalFfmpegPath ?? '';
	} catch (error) {
		appSettingsDialogState.settings = null;
		appSettingsDialogState.saveState = 'error';
		appSettingsDialogState.saveError = describeError(error);
	} finally {
		appSettingsDialogState.loading = false;
	}
	await refreshEncoderAvailability();
}

async function refreshEncoderAvailability(): Promise<void> {
	const capabilities = await refreshRuntimeSettingsCapabilities();
	appSettingsDialogState.encoderAvailability = capabilities?.encoder?.availability ?? null;
}

export async function browseForFfmpegBinary(): Promise<void> {
	const selected = await tauriClient.openFile({
		title: 'Choose an FFmpeg binary with libfdk_aac',
	});
	if (selected) {
		appSettingsDialogState.ffmpegPathDraft = selected;
	}
}

export function clearFfmpegPathDraft(): void {
	appSettingsDialogState.ffmpegPathDraft = '';
}

/// Persists the FFmpeg path preference, then refreshes encoder capabilities so
/// the dialog reflects the toolchain owner's validation verdict immediately.
export async function saveToolchainPreference(): Promise<void> {
	appSettingsDialogState.saveState = 'saving';
	appSettingsDialogState.saveError = '';
	const draft = appSettingsDialogState.ffmpegPathDraft.trim();
	try {
		const settings = await tauriClient.updateAppSettings({
			toolchain: { externalFfmpegPath: draft.length > 0 ? draft : undefined },
		});
		appSettingsDialogState.settings = settings;
		appSettingsDialogState.ffmpegPathDraft = settings.toolchain?.externalFfmpegPath ?? '';
		appSettingsDialogState.saveState = 'saved';
	} catch (error) {
		appSettingsDialogState.saveState = 'error';
		appSettingsDialogState.saveError = describeError(error);
	}
	await refreshEncoderAvailability();
}

/// Resets every durable preference to defaults, then re-hydrates the owning
/// controls so runtime state follows the reset settings.
export async function resetAllAppSettings(): Promise<void> {
	appSettingsDialogState.saveState = 'saving';
	appSettingsDialogState.saveError = '';
	try {
		await tauriClient.resetAppSettings();
		await hydrateAppSettings();
		appSettingsDialogState.saveState = 'saved';
	} catch (error) {
		appSettingsDialogState.saveState = 'error';
		appSettingsDialogState.saveError = describeError(error);
	}
	await reloadDialogData();
}

function describeError(error: unknown): string {
	return toUserMessage(error, { fallback: 'Settings update failed.' });
}
