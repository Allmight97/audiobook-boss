import { afterEach, describe, expect, it } from 'vitest';
import {
	FIXTURE_AUDIO_PATHS,
	FIXTURE_CHAPTER_1,
	FIXTURE_INVALID,
	FIXTURE_LIBRARY_DIR,
	FIXTURE_OUTPUT_DIR,
	MOCK_ENCODE_ERROR,
} from './fixtures';
import {
	MOCK_SCENARIO_IDS,
	applyScenario,
	handleInvoke,
	openCannedDialog,
	resetMockRuntime,
} from './runtime';

const MOCK_COMMANDS = [
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

afterEach(() => {
	resetMockRuntime();
});

describe('mock runtime invoke handlers', () => {
	it('answers every public-strip command without invoking Rust', () => {
		const results = MOCK_COMMANDS.map((cmd) => {
			try {
				return { cmd, ok: true, value: handleInvoke(cmd, samplePayload(cmd)) };
			} catch (error) {
				return { cmd, ok: false, error };
			}
		});

		const hardFailures = results.filter(
			(result) =>
				!result.ok &&
				typeof result.error === 'object' &&
				result.error !== null &&
				'message' in result.error &&
				String((result.error as { message: string }).message).includes('no handler'),
		);
		expect(hardFailures).toEqual([]);
		expect(results).toHaveLength(MOCK_COMMANDS.length);
	});

	it('returns canned dialog paths and never live filesystem roots', () => {
		expect(openCannedDialog({ multiple: true, directory: false })).toEqual([
			...FIXTURE_AUDIO_PATHS,
		]);
		expect(openCannedDialog({ directory: true, title: 'Add Folder' })).toBe(FIXTURE_LIBRARY_DIR);
		expect(openCannedDialog({ directory: true, title: 'Select Output Directory' })).toBe(
			FIXTURE_OUTPUT_DIR,
		);
		expect(handleInvoke('plugin:dialog|open', { options: { multiple: true } })).toEqual([
			...FIXTURE_AUDIO_PATHS,
		]);
	});

	it('drains opened files once after a files-loaded scenario', () => {
		applyScenario('files-loaded');
		expect(handleInvoke('take_opened_audio_files')).toEqual([...FIXTURE_AUDIO_PATHS]);
		expect(handleInvoke('take_opened_audio_files')).toEqual([]);
	});

	it('seeds a running work operation for encoding-in-progress', () => {
		applyScenario('encoding-in-progress');
		const listed = handleInvoke('list_work_operations') as {
			operations: Array<{ status: string }>;
		};
		expect(listed.operations[0]?.status).toBe('running');
	});

	it('fails encode in the error scenario with a typed envelope', () => {
		applyScenario('error');
		expect(handleInvoke('take_opened_audio_files')).toContain(FIXTURE_INVALID);
		expect(
			invokeError('submit_processing_operation', {
				request: {
					payload: { inputFiles: [...FIXTURE_AUDIO_PATHS], jobType: 'batch' },
				},
			}),
		).toMatchObject(MOCK_ENCODE_ERROR);
		expect(
			invokeError('process_audiobook_files', {
				payload: { inputFiles: [FIXTURE_CHAPTER_1], jobType: 'batch' },
			}),
		).toMatchObject(MOCK_ENCODE_ERROR);
	});

	it('keeps Audible logged out in the dedicated scenario', () => {
		applyScenario('audible-logged-out');
		expect(
			handleInvoke('get_remote_source_account_state', { providerId: 'audible' }),
		).toMatchObject({
			providerId: 'audible',
			status: 'needsAuth',
		});
		expect(handleInvoke('take_opened_audio_files')).toEqual([]);
	});

	it('discovers fixture files from the canned library folder', () => {
		expect(
			handleInvoke('discover_audio_import_paths', { inputPaths: [FIXTURE_LIBRARY_DIR] }),
		).toEqual([...FIXTURE_AUDIO_PATHS]);
	});

	it('pins the scenario list the switcher exposes', () => {
		expect([...MOCK_SCENARIO_IDS]).toEqual([
			'empty',
			'files-loaded',
			'encoding-in-progress',
			'error',
			'audible-logged-out',
		]);
	});
});

function invokeError(cmd: string, payload: Record<string, unknown>): unknown {
	try {
		handleInvoke(cmd, payload);
		throw new Error(`expected ${cmd} to fail`);
	} catch (error) {
		return error;
	}
}

function samplePayload(cmd: string): Record<string, unknown> {
	switch (cmd) {
		case 'analyze_audio_files':
			return { filePaths: [...FIXTURE_AUDIO_PATHS] };
		case 'discover_audio_import_paths':
			return { inputPaths: [FIXTURE_LIBRARY_DIR] };
		case 'read_audio_metadata':
		case 'read_audio_cover_thumbnail':
		case 'write_cover_art':
		case 'load_cover_art_file':
		case 'save_metadata_to_file':
			return { filePath: FIXTURE_CHAPTER_1, coverData: [], metadataPatch: {} };
		case 'load_cover_art_from_url':
			return { url: 'https://mock.invalid/cover.jpg' };
		case 'validate_files':
			return { filePaths: [...FIXTURE_AUDIO_PATHS] };
		case 'validate_metadata_intent_patch':
			return { metadataPatch: { title: { op: 'set', value: 'Dune' } } };
		case 'save_metadata_batch':
			return { items: [{ filePath: FIXTURE_CHAPTER_1, metadataPatch: {} }] };
		case 'search_online_metadata':
			return { query: 'Dune', sources: null, limit: 5 };
		case 'preview_output_path':
			return { outputDir: FIXTURE_OUTPUT_DIR };
		case 'preflight_processing_plan':
		case 'process_audiobook_files':
			return { payload: { inputFiles: [...FIXTURE_AUDIO_PATHS], jobType: 'batch' } };
		case 'submit_processing_operation':
			return { request: { payload: { inputFiles: [...FIXTURE_AUDIO_PATHS], jobType: 'batch' } } };
		case 'get_work_operation':
		case 'cancel_work_operation':
			return { operationId: 'mock-operation-1' };
		case 'set_max_concurrent_jobs':
			return { maxConcurrent: 2 };
		case 'update_app_settings':
			return { patch: { startupBehavior: 'rememberLastState' } };
		case 'get_remote_source_account_state':
		case 'start_remote_source_auth':
		case 'logout_remote_source_account':
		case 'load_remote_source_library':
			return { providerId: 'audible' };
		case 'complete_remote_source_auth':
			return { request: { providerId: 'audible', responseUrlHandoffPath: null } };
		case 'start_remote_source_acquisition':
		case 'get_remote_source_acquisition_status':
		case 'cancel_remote_source_acquisition':
		case 'purge_remote_source_session':
			return { jobId: 'mock-job', plan: { providerId: 'audible', selections: [] } };
		case 'validate_encoder_settings':
			return { settings: { encoderType: 'auto' } };
		case 'log_frontend':
			return { entry: { level: 'warn', scope: 'mock', message: 'fixture' } };
		default:
			return {};
	}
}
