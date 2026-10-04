import { listen as tauriListen, type UnlistenFn } from '@tauri-apps/api/event';
import {
	ask as tauriAsk,
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
	type OpenedAudioFilesEvent,
	type SessionUpdateEvent,
	type SettingsUpdateEvent,
	type WorkOperationListSnapshotEvent,
	type WorkOperationSnapshotEvent,
} from '../../types/events';
import type { SettingsIntent } from '../../types/appSettings';
import type { FrontendLogEntry } from '../../types/frontendLog';
import type { SessionIntent } from '../../types/session';
import type {
	OperationId,
	OperationListSnapshot,
	OperationSnapshot,
} from '../../types/workRuntime';
import { commandSpecs, type CommandResult, type TauriCommand } from './commands';
import {
	normalizeOperationListSnapshot,
	normalizeOperationSnapshot,
	normalizeSessionUpdate,
	normalizeSettingsSnapshot,
} from './normalizers';

type AppEventName = (typeof TAURI_APP_EVENT_NAMES)[number];
type RuntimeEventName = Exclude<EventName, AppEventName>;
type OpenedAudioFilesHandler = (event: { payload: OpenedAudioFilesEvent }) => void;
type WorkOperationSnapshotHandler = (event: { payload: WorkOperationSnapshotEvent }) => void;
type WorkOperationListSnapshotHandler = (event: {
	payload: WorkOperationListSnapshotEvent;
}) => void;
type SessionUpdateHandler = (event: { payload: SessionUpdateEvent }) => void;
type SettingsUpdateHandler = (event: { payload: SettingsUpdateEvent }) => void;

type DialogOptions = Omit<OpenDialogOptions, 'multiple' | 'directory'>;

async function listenOpenedAudioFiles(handler: OpenedAudioFilesHandler): Promise<UnlistenFn> {
	return generatedEvents.openedAudioFiles.listen((event) => {
		handler({ payload: event.payload });
	});
}

async function listenWorkOperationSnapshot(
	handler: WorkOperationSnapshotHandler,
): Promise<UnlistenFn> {
	return generatedEvents.workOperationSnapshot.listen((event) => {
		handler({ payload: { snapshot: normalizeOperationSnapshot(event.payload.snapshot) } });
	});
}

async function listenWorkOperationListSnapshot(
	handler: WorkOperationListSnapshotHandler,
): Promise<UnlistenFn> {
	return generatedEvents.workOperationListSnapshot.listen((event) => {
		handler({
			payload: normalizeOperationListSnapshot(event.payload),
		});
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
	event: typeof EVENTS.OPENED_AUDIO_FILES,
	handler: OpenedAudioFilesHandler,
): Promise<UnlistenFn>;
function listen(
	event: typeof EVENTS.WORK_OPERATION_SNAPSHOT,
	handler: WorkOperationSnapshotHandler,
): Promise<UnlistenFn>;
function listen(
	event: typeof EVENTS.WORK_OPERATION_LIST_SNAPSHOT,
	handler: WorkOperationListSnapshotHandler,
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
		| OpenedAudioFilesHandler
		| WorkOperationSnapshotHandler
		| WorkOperationListSnapshotHandler
		| SessionUpdateHandler
		| SettingsUpdateHandler
		| ((event: { payload: ApplicationEvents[RuntimeEventName] }) => void),
): Promise<UnlistenFn> {
	if (event === EVENTS.OPENED_AUDIO_FILES) {
		return listenOpenedAudioFiles(handler as OpenedAudioFilesHandler);
	}

	if (event === EVENTS.WORK_OPERATION_SNAPSHOT) {
		return listenWorkOperationSnapshot(handler as WorkOperationSnapshotHandler);
	}

	if (event === EVENTS.WORK_OPERATION_LIST_SNAPSHOT) {
		return listenWorkOperationListSnapshot(handler as WorkOperationListSnapshotHandler);
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
	/** Asks a yes/no question in a native dialog; resolves `true` for the OK button. */
	ask: (
		message: string,
		options: { title: string; okLabel: string; cancelLabel: string },
	): Promise<boolean> => tauriAsk(message, { ...options, kind: 'warning' }),
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
	sessionCoverArt: (): Promise<CommandResult<'session_cover_art'>> =>
		commandSpecs.session_cover_art(),
	readAudioMetadata: (filePath: string): Promise<CommandResult<'read_audio_metadata'>> =>
		commandSpecs.read_audio_metadata({ filePath }),
	loadCoverArtFromUrl: (url: string): Promise<CommandResult<'load_cover_art_from_url'>> =>
		commandSpecs.load_cover_art_from_url({ url }),
	readAudioCoverThumbnail: (
		filePath: string,
	): Promise<CommandResult<'read_audio_cover_thumbnail'>> =>
		commandSpecs.read_audio_cover_thumbnail({ filePath }),
	getSupportedAudioImportMetadata: (): Promise<
		CommandResult<'get_supported_audio_import_metadata'>
	> => commandSpecs.get_supported_audio_import_metadata(),
	listWorkOperations: (): Promise<OperationListSnapshot> => commandSpecs.list_work_operations(),
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
	'opened-audio-files',
	'work-operation-snapshot',
	'work-operation-list-snapshot',
	'session-update',
	'settings-update',
] as const);

export type { TauriCommand };
