import { describe, expect, it } from 'vitest';

import { tauriClient, TAURI_APP_EVENT_NAMES, TAURI_COMMAND_NAMES } from './tauri/client';

const EXPECTED_TAURI_CLIENT_METHODS = [
	'analyzeAudioFiles',
	'cancelProcessing',
	'echo',
	'getMaxConcurrentJobs',
	'listAvailableEncoders',
	'listen',
	'loadCoverArtFile',
	'loadCoverArtFromUrl',
	'open',
	'openExternal',
	'ping',
	'preflightProcessingPlan',
	'previewOutputPath',
	'processAudiobookFiles',
	'readAudioMetadata',
	'refreshExternalToolchain',
	'saveMetadataBatch',
	'saveMetadataIntentToFile',
	'searchOnlineMetadata',
	'setMaxConcurrentJobs',
	'validateEncoderSettings',
	'validateFiles',
	'writeCoverArt',
] as const;

describe('Tauri Runtime Boundary public API contract', () => {
	it('pins the tauriClient public method strip', () => {
		expect(Object.keys(tauriClient).sort()).toEqual([...EXPECTED_TAURI_CLIENT_METHODS].sort());
	});

	it('keeps command and app event names exposed through the boundary', () => {
		expect(TAURI_COMMAND_NAMES).toContain('process_audiobook_files');
		expect(TAURI_COMMAND_NAMES).toContain('preflight_processing_plan');
		expect(TAURI_COMMAND_NAMES).toContain('save_metadata_batch');
		expect([...TAURI_APP_EVENT_NAMES]).toEqual(['processing-progress', 'processing-queue']);
	});
});
