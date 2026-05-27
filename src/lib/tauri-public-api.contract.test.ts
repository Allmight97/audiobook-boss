import { describe, expect, it } from 'vitest';

import { tauriClient, TAURI_APP_EVENT_NAMES, TAURI_COMMAND_NAMES } from './tauri/client';

const EXPECTED_TAURI_CLIENT_METHODS = [
	'analyzeAudioFiles',
	'cancelProcessing',
	'discoverAudioImportPaths',
	'echo',
	'getAppSettings',
	'getMaxConcurrentJobs',
	'getRuntimeSettingsCapabilities',
	'getSupportedAudioImportMetadata',
	'listAvailableEncoders',
	'listen',
	'loadCoverArtFile',
	'loadCoverArtFromUrl',
	'open',
	'openDirectory',
	'openFile',
	'openFiles',
	'openPath',
	'openUrl',
	'ping',
	'preflightProcessingPlan',
	'previewOutputPath',
	'processAudiobookFiles',
	'readAudioMetadata',
	'refreshExternalToolchain',
	'resetAppSettings',
	'saveMetadataBatch',
	'saveMetadataIntentToFile',
	'searchOnlineMetadata',
	'setMaxConcurrentJobs',
	'takeOpenedAudioFiles',
	'updateAppSettings',
	'validateEncoderSettings',
	'validateFiles',
	'validateMetadataIntentPatch',
	'writeCoverArt',
] as const;

describe('Tauri Runtime Boundary public API contract', () => {
	it('pins the tauriClient public method strip', () => {
		expect(Object.keys(tauriClient).sort()).toEqual([...EXPECTED_TAURI_CLIENT_METHODS].sort());
	});

	it('keeps command and app event names exposed through the boundary', () => {
		expect(TAURI_COMMAND_NAMES).toContain('process_audiobook_files');
		expect(TAURI_COMMAND_NAMES).toContain('preflight_processing_plan');
		expect(TAURI_COMMAND_NAMES).toContain('get_app_settings');
		expect(TAURI_COMMAND_NAMES).toContain('update_app_settings');
		expect(TAURI_COMMAND_NAMES).toContain('reset_app_settings');
		expect(TAURI_COMMAND_NAMES).toContain('get_runtime_settings_capabilities');
		expect(TAURI_COMMAND_NAMES).toContain('validate_metadata_intent_patch');
		expect(TAURI_COMMAND_NAMES).toContain('save_metadata_batch');
		expect(TAURI_COMMAND_NAMES).toContain('discover_audio_import_paths');
		expect(TAURI_COMMAND_NAMES).toContain('get_supported_audio_import_metadata');
		expect(TAURI_COMMAND_NAMES).toContain('take_opened_audio_files');
		expect([...TAURI_APP_EVENT_NAMES]).toEqual([
			'processing-progress',
			'processing-queue',
			'opened-audio-files',
		]);
	});
});
