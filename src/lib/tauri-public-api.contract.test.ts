import { describe, expect, it } from 'vitest';

import { tauriClient, TAURI_APP_EVENT_NAMES, TAURI_COMMAND_NAMES } from './tauri/client';

const EXPECTED_TAURI_CLIENT_METHODS = [
	'analyzeAudioFiles',
	'cancelRemoteSourceAcquisition',
	'cancelWorkOperation',
	'completeRemoteSourceAuth',
	'discoverAudioImportPaths',
	'getAppSettings',
	'getMaxConcurrentJobs',
	'getRemoteSourceAccountState',
	'getRemoteSourceAcquisitionStatus',
	'getRuntimeSettingsCapabilities',
	'getSupportedAudioImportMetadata',
	'getWorkOperation',
	'listen',
	'listRemoteSourceProviders',
	'listWorkOperations',
	'loadCoverArtFile',
	'loadCoverArtFromUrl',
	'loadRemoteSourceLibrary',
	'logFrontend',
	'logoutRemoteSourceAccount',
	'open',
	'openDirectory',
	'openFile',
	'openFiles',
	'openPath',
	'openUrl',
	'preflightProcessingPlan',
	'previewOutputPath',
	'processAudiobookFiles',
	'purgeRemoteSourceSession',
	'readAudioMetadata',
	'readAudioCoverThumbnail',
	'resetAppSettings',
	'saveMetadataBatch',
	'saveMetadataIntentToFile',
	'searchOnlineMetadata',
	'setMaxConcurrentJobs',
	'startRemoteSourceAcquisition',
	'startRemoteSourceAuth',
	'submitProcessingOperation',
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
		expect(TAURI_COMMAND_NAMES).toContain('read_audio_cover_thumbnail');
		expect(TAURI_COMMAND_NAMES).toContain('save_metadata_batch');
		expect(TAURI_COMMAND_NAMES).toContain('discover_audio_import_paths');
		expect(TAURI_COMMAND_NAMES).toContain('get_supported_audio_import_metadata');
		expect(TAURI_COMMAND_NAMES).toContain('take_opened_audio_files');
		expect(TAURI_COMMAND_NAMES).toContain('list_remote_source_providers');
		expect(TAURI_COMMAND_NAMES).toContain('start_remote_source_acquisition');
		expect(TAURI_COMMAND_NAMES).toContain('submit_processing_operation');
		expect(TAURI_COMMAND_NAMES).toContain('list_work_operations');
		expect(TAURI_COMMAND_NAMES).toContain('get_work_operation');
		expect(TAURI_COMMAND_NAMES).toContain('cancel_work_operation');
		expect(TAURI_COMMAND_NAMES).toContain('log_frontend');
		expect([...TAURI_APP_EVENT_NAMES]).toEqual([
			'processing-progress',
			'processing-queue',
			'opened-audio-files',
			'work-operation-snapshot',
			'work-operation-list-snapshot',
		]);
	});
});
