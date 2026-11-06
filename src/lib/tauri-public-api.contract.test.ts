import { describe, expect, it } from 'vitest';

// oxlint-disable-next-line import/default -- Vite's ?raw import exports the file text as its default.
import generated from './generated/tauri.ts?raw';
import { tauriClient, TAURI_APP_EVENT_NAMES, TAURI_COMMAND_NAMES } from './tauri/client';

const EXPECTED_COMMAND_NAMES = [
	'attach_frontend',
	'session_dispatch',
	'open_preview_output',
	'settings_dispatch',
	'cancel_work_operation',
	'get_supported_audio_import_metadata',
	'list_work_operations',
	'log_frontend',
] as const;

const EXPECTED_APP_EVENT_NAMES = [
	'work-operations-update',
	'session-update',
	'settings-update',
] as const;

const EXPECTED_TAURI_CLIENT_METHODS = [
	'attachFrontend',
	'sessionDispatch',
	'openPreviewOutput',
	'settingsDispatch',
	'cancelWorkOperation',
	'getSupportedAudioImportMetadata',
	'listen',
	'listWorkOperations',
	'logFrontend',
	'openDirectory',
	'openFile',
	'openFiles',
	'revealPath',
	'openUrl',
] as const;

describe('Tauri Runtime Boundary public API contract', () => {
	it('pins the tauriClient public method strip', () => {
		expect(Object.keys(tauriClient).toSorted()).toEqual(
			[...EXPECTED_TAURI_CLIENT_METHODS].toSorted(),
		);
	});

	it('pins the independent command and app-event strips', () => {
		expect([...TAURI_COMMAND_NAMES].toSorted()).toEqual([...EXPECTED_COMMAND_NAMES].toSorted());
		expect([...TAURI_APP_EVENT_NAMES]).toEqual([...EXPECTED_APP_EVENT_NAMES]);
	});

	it('every Rust command has a commandSpecs entry', () => {
		const invoked = [...generated.matchAll(/__TAURI_INVOKE\("(\w+)"/g)].map((match) => match[1]);
		expect(invoked.toSorted()).toEqual([...TAURI_COMMAND_NAMES].toSorted());
	});
});
