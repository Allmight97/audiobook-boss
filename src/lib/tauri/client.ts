import { listen as tauriListen } from '@tauri-apps/api/event';
import { open as tauriOpen, type OpenDialogOptions } from '@tauri-apps/plugin-dialog';
import { openUrl as tauriOpenUrl, revealItemInDir } from '@tauri-apps/plugin-opener';

import { events as generatedEvents } from '../generated/tauri';
import type {
	FrontendLogEntry,
	OperationId,
	SessionIntent,
	SettingsIntent,
} from '../generated/tauri';
import { commandSpecs, type CommandResult, type TauriCommand } from './commands';

type DialogOptions = Omit<OpenDialogOptions, 'multiple' | 'directory'>;

function openFile(options?: DialogOptions): Promise<string | null> {
	return tauriOpen({ ...options, multiple: false, directory: false } as const);
}

function openFiles(options?: DialogOptions): Promise<string[] | null> {
	return tauriOpen({ ...options, multiple: true, directory: false } as const);
}

function openDirectory(options?: DialogOptions): Promise<string | null> {
	return tauriOpen({ ...options, multiple: false, directory: true } as const);
}

export const tauriClient = {
	attachFrontend: (): Promise<CommandResult<'attach_frontend'>> => commandSpecs.attach_frontend(),
	/**
	 * Applies one session intent. `sequence` counts this frontend's session
	 * intents from zero; the host applies them in that order.
	 */
	sessionDispatch: (
		client: number,
		sequence: number,
		intent: SessionIntent,
	): Promise<CommandResult<'session_dispatch'>> =>
		commandSpecs.session_dispatch({ client, sequence, intent }),
	/**
	 * Claims a finished preview's output and opens it in the host, numbered
	 * with the session intents.
	 */
	openPreviewOutput: (
		client: number,
		sequence: number,
		runId: string,
	): Promise<CommandResult<'open_preview_output'>> =>
		commandSpecs.open_preview_output({ client, sequence, runId }),
	/** Applies one settings intent; numbered separately from session intents. */
	settingsDispatch: (
		client: number,
		sequence: number,
		intent: SettingsIntent,
	): Promise<CommandResult<'settings_dispatch'>> =>
		commandSpecs.settings_dispatch({ client, sequence, intent }),
	/** Cancels the whole operation, or only the title named by `childJobId`. */
	cancelWorkOperation: (operationId: OperationId, childJobId?: string): Promise<void> =>
		commandSpecs.cancel_work_operation({ operationId, childJobId }).then(() => undefined),
	logFrontend: (entry: FrontendLogEntry): Promise<void> =>
		commandSpecs.log_frontend({ entry }).then(() => undefined),
	events: generatedEvents,
	listen: tauriListen,
	openFile,
	openFiles,
	openDirectory,
	revealPath: (path: string): Promise<void> => revealItemInDir(path),
	openUrl: (url: string | URL): Promise<void> => tauriOpenUrl(url),
};

// oxlint-disable-next-line typescript/no-unsafe-type-assertion -- Object.keys returns string[]; commandSpecs is keyed by TauriCommand.
export const TAURI_COMMAND_NAMES = Object.freeze(
	Object.keys(commandSpecs),
) as readonly TauriCommand[];

export type { TauriCommand };
