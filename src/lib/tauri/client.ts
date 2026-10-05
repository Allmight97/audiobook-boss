import { listen as tauriListen, type UnlistenFn } from '@tauri-apps/api/event';
import {
	open as tauriOpen,
	type OpenDialogOptions,
	type OpenDialogReturn,
} from '@tauri-apps/plugin-dialog';
import {
	openPath as tauriOpenPath,
	openUrl as tauriOpenUrl,
	revealItemInDir,
} from '@tauri-apps/plugin-opener';

import { events as generatedEvents } from '../generated/tauri';
import {
	EVENTS,
	type ApplicationEvents,
	type EventName,
	type SessionUpdateEvent,
	type SettingsUpdateEvent,
	type WorkOperationsUpdateEvent,
} from '../../types/events';
import type { SettingsIntent } from '../../types/appSettings';
import type { FrontendLogEntry } from '../../types/frontendLog';
import type { SessionIntent } from '../../types/session';
import type {
	OperationId,
	OperationSnapshot,
	WorkOperationsSnapshot,
} from '../../types/workRuntime';
import { commandSpecs, type CommandResult, type TauriCommand } from './commands';
import {
	normalizeWorkOperationsUpdate,
	normalizeSessionUpdate,
	normalizeSettingsSnapshot,
} from './normalizers';

type AppEventName = (typeof TAURI_APP_EVENT_NAMES)[number];
type RuntimeEventName = Exclude<EventName, AppEventName>;
type WorkOperationsUpdateHandler = (event: { payload: WorkOperationsUpdateEvent }) => void;
type SessionUpdateHandler = (event: { payload: SessionUpdateEvent }) => void;
type SettingsUpdateHandler = (event: { payload: SettingsUpdateEvent }) => void;

type DialogOptions = Omit<OpenDialogOptions, 'multiple' | 'directory'>;

async function listenWorkOperationsUpdate(
	handler: WorkOperationsUpdateHandler,
): Promise<UnlistenFn> {
	return generatedEvents.workOperationsUpdate.listen((event) => {
		handler({ payload: normalizeWorkOperationsUpdate(event.payload) });
	});
}

async function listenSettingsUpdate(handler: SettingsUpdateHandler): Promise<UnlistenFn> {
	return generatedEvents.settingsUpdate.listen((event) => {
		handler({ payload: normalizeSettingsSnapshot(event.payload) });
	});
}

async function listenSessionUpdate(handler: SessionUpdateHandler): Promise<UnlistenFn> {
	return generatedEvents.sessionUpdate.listen((event) => {
		handler({ payload: normalizeSessionUpdate(event.payload) });
	});
}

function listen(
	event: typeof EVENTS.WORK_OPERATIONS_UPDATE,
	handler: WorkOperationsUpdateHandler,
): Promise<UnlistenFn>;
function listen(
	event: typeof EVENTS.SESSION_UPDATE,
	handler: SessionUpdateHandler,
): Promise<UnlistenFn>;
function listen(
	event: typeof EVENTS.SETTINGS_UPDATE,
	handler: SettingsUpdateHandler,
): Promise<UnlistenFn>;
function listen<E extends RuntimeEventName>(
	event: E,
	handler: (event: { payload: ApplicationEvents[E] }) => void,
): Promise<UnlistenFn>;
function listen(
	event: EventName,
	handler:
		| WorkOperationsUpdateHandler
		| SessionUpdateHandler
		| SettingsUpdateHandler
		| ((event: { payload: ApplicationEvents[RuntimeEventName] }) => void),
): Promise<UnlistenFn> {
	if (event === EVENTS.WORK_OPERATIONS_UPDATE) {
		return listenWorkOperationsUpdate(handler as WorkOperationsUpdateHandler);
	}

	if (event === EVENTS.SESSION_UPDATE) {
		return listenSessionUpdate(handler as SessionUpdateHandler);
	}

	if (event === EVENTS.SETTINGS_UPDATE) {
		return listenSettingsUpdate(handler as SettingsUpdateHandler);
	}

	return tauriListen(
		event,
		handler as (event: { payload: ApplicationEvents[RuntimeEventName] }) => void,
	);
}

function openDialog<T extends OpenDialogOptions>(options: T): Promise<OpenDialogReturn<T>>;
function openDialog(): Promise<string | string[] | null>;
function openDialog<T extends OpenDialogOptions>(
	options?: T,
): Promise<OpenDialogReturn<T> | string | string[] | null> {
	return tauriOpen(options) as Promise<OpenDialogReturn<T>>;
}

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
	/** Attaches this frontend and returns the whole session and settings. */
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
	/** Applies one settings intent; numbered separately from session intents. */
	settingsDispatch: (
		client: number,
		sequence: number,
		intent: SettingsIntent,
	): Promise<CommandResult<'settings_dispatch'>> =>
		commandSpecs.settings_dispatch({ client, sequence, intent }),
	getSupportedAudioImportMetadata: (): Promise<
		CommandResult<'get_supported_audio_import_metadata'>
	> => commandSpecs.get_supported_audio_import_metadata(),
	listWorkOperations: (): Promise<WorkOperationsSnapshot> => commandSpecs.list_work_operations(),
	/** Cancels the whole operation, or only the title named by `childJobId`. */
	cancelWorkOperation: (
		operationId: OperationId,
		childJobId?: string,
	): Promise<OperationSnapshot> => commandSpecs.cancel_work_operation({ operationId, childJobId }),
	logFrontend: (entry: FrontendLogEntry): Promise<void> =>
		commandSpecs.log_frontend({ entry }).then(() => undefined),
	listen,
	open: openDialog,
	openFile,
	openFiles,
	openDirectory,
	openPath: (path: string, openWith?: string): Promise<void> => tauriOpenPath(path, openWith),
	/** Shows the file selected in the host OS file manager. */
	revealPath: (path: string): Promise<void> => revealItemInDir(path),
	openUrl: (url: string | URL, openWith?: string): Promise<void> => tauriOpenUrl(url, openWith),
};

export const TAURI_COMMAND_NAMES = Object.freeze(
	Object.keys(commandSpecs),
) as readonly TauriCommand[];

export const TAURI_APP_EVENT_NAMES = Object.freeze([
	'work-operations-update',
	'session-update',
	'settings-update',
] as const);

export type { TauriCommand };
