import { describe, expect, it } from 'vitest';

import { tauriClient, TAURI_APP_EVENT_NAMES, TAURI_COMMAND_NAMES } from './tauri/client';

const EXPECTED_COMMAND_NAMES = [
	'attach_frontend',
	'session_cover_art',
	'session_dispatch',
	'settings_dispatch',
	'cancel_work_operation',
	'get_supported_audio_import_metadata',
	'list_work_operations',
	'log_frontend',
	'load_cover_art_from_url',
	'read_audio_cover_thumbnail',
] as const;

const EXPECTED_APP_EVENT_NAMES = [
	'work-operations-update',
	'session-update',
	'settings-update',
] as const;

const EXPECTED_TAURI_CLIENT_METHODS = [
	'attachFrontend',
	'sessionCoverArt',
	'sessionDispatch',
	'settingsDispatch',
	'cancelWorkOperation',
	'getSupportedAudioImportMetadata',
	'listen',
	'listWorkOperations',
	'logFrontend',
	'loadCoverArtFromUrl',
	'open',
	'openDirectory',
	'openFile',
	'openFiles',
	'openPath',
	'revealPath',
	'openUrl',
	'readAudioCoverThumbnail',
] as const;

describe('Tauri Runtime Boundary public API contract', () => {
	it('pins the tauriClient public method strip', () => {
		expect(Object.keys(tauriClient).sort()).toEqual([...EXPECTED_TAURI_CLIENT_METHODS].sort());
	});

	it('pins the independent command and app-event strips', () => {
		expect([...TAURI_COMMAND_NAMES].sort()).toEqual([...EXPECTED_COMMAND_NAMES].sort());
		expect([...TAURI_APP_EVENT_NAMES]).toEqual([...EXPECTED_APP_EVENT_NAMES]);
	});
});
