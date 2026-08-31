import { describe, expect, it } from 'vitest';

import { tauriClient, TAURI_APP_EVENT_NAMES, TAURI_COMMAND_NAMES } from './tauri/client';

const EXPECTED_COMMAND_NAMES = [
	'analyze_audio_files',
	'cancel_remote_source_acquisition',
	'cancel_work_operation',
	'complete_remote_source_auth',
	'discover_audio_import_paths',
	'get_app_settings',
	'get_max_concurrent_jobs',
	'get_remote_source_account_state',
	'get_remote_source_acquisition_status',
	'get_runtime_settings_capabilities',
	'get_supported_audio_import_metadata',
	'get_work_operation',
	'list_remote_source_providers',
	'list_work_operations',
	'log_frontend',
	'load_cover_art_file',
	'load_cover_art_from_url',
	'preview_cover_art_from_url',
	'load_remote_source_library',
	'logout_remote_source_account',
	'preflight_processing_plan',
	'preview_output_path',
	'process_audiobook_files',
	'purge_remote_source_session',
	'read_audio_cover_thumbnail',
	'read_audio_metadata',
	'reset_app_settings',
	'save_metadata_batch',
	'save_metadata_to_file',
	'search_online_metadata',
	'set_max_concurrent_jobs',
	'start_remote_source_acquisition',
	'start_remote_source_auth',
	'submit_processing_operation',
	'take_opened_audio_files',
	'update_app_settings',
	'validate_encoder_settings',
	'validate_files',
	'validate_metadata_intent_patch',
	'write_cover_art',
] as const;

const EXPECTED_APP_EVENT_NAMES = [
	'processing-progress',
	'processing-queue',
	'opened-audio-files',
	'work-operation-snapshot',
	'work-operation-list-snapshot',
] as const;

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
	'logFrontend',
	'loadCoverArtFile',
	'loadCoverArtFromUrl',
	'previewCoverArtFromUrl',
	'loadRemoteSourceLibrary',
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

	it('pins the independent command and app-event strips', () => {
		expect([...TAURI_COMMAND_NAMES].sort()).toEqual([...EXPECTED_COMMAND_NAMES].sort());
		expect([...TAURI_APP_EVENT_NAMES]).toEqual([...EXPECTED_APP_EVENT_NAMES]);
	});
});
