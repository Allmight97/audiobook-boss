import { beforeEach, describe, expect, it, vi } from 'vitest';

const bootstrapDeps = vi.hoisted(() => ({
	displayFileList: vi.fn(),
	selectFile: vi.fn(),
	updateOutputDirectory: vi.fn(),
	updateNamingPreset: vi.fn(),
	initJobControls: vi.fn(),
	setJobTypeSelection: vi.fn(),
	clearMetadataState: vi.fn(),
	setMetadataForFile: vi.fn(),
}));

vi.mock('../ui/fileList/actions', () => ({
	displayFileList: bootstrapDeps.displayFileList,
	selectFile: bootstrapDeps.selectFile,
}));

vi.mock('../ui/outputPanel/state', () => ({
	updateOutputDirectory: bootstrapDeps.updateOutputDirectory,
	updateNamingPreset: bootstrapDeps.updateNamingPreset,
}));

vi.mock('../ui/jobControls', () => ({
	initJobControls: bootstrapDeps.initJobControls,
	setJobTypeSelection: bootstrapDeps.setJobTypeSelection,
}));

vi.mock('../ui/metadataState', () => ({
	clearMetadataState: bootstrapDeps.clearMetadataState,
	setMetadataForFile: bootstrapDeps.setMetadataForFile,
}));

import {
	HARNESS_FILE_LIST,
	HARNESS_METADATA_BY_FILE,
	HARNESS_OUTPUT_DIRECTORY,
} from './sampleData';
import { bootstrapHarnessRuntime } from './bootstrap';

describe('bootstrapHarnessRuntime', () => {
	beforeEach(() => {
		bootstrapDeps.displayFileList.mockReset();
		bootstrapDeps.selectFile.mockReset();
		bootstrapDeps.selectFile.mockResolvedValue(undefined);
		bootstrapDeps.updateOutputDirectory.mockReset();
		bootstrapDeps.updateNamingPreset.mockReset();
		bootstrapDeps.initJobControls.mockReset();
		bootstrapDeps.setJobTypeSelection.mockReset();
		bootstrapDeps.clearMetadataState.mockReset();
		bootstrapDeps.setMetadataForFile.mockReset();
	});

	it('seeds harness state without duplicating job-controls init', async () => {
		await bootstrapHarnessRuntime();

		expect(bootstrapDeps.initJobControls).not.toHaveBeenCalled();
		expect(bootstrapDeps.updateOutputDirectory).toHaveBeenCalledWith(HARNESS_OUTPUT_DIRECTORY);
		expect(bootstrapDeps.updateNamingPreset).toHaveBeenCalledWith('absDefault');
		expect(bootstrapDeps.setJobTypeSelection).toHaveBeenCalledWith('batch');
		expect(bootstrapDeps.displayFileList).toHaveBeenCalledWith(HARNESS_FILE_LIST);
		expect(bootstrapDeps.clearMetadataState).toHaveBeenCalledTimes(1);
		expect(bootstrapDeps.setMetadataForFile.mock.calls).toEqual(
			Object.entries(HARNESS_METADATA_BY_FILE),
		);
		expect(bootstrapDeps.selectFile).toHaveBeenCalledWith(
			0,
			{ multi: false, range: false },
			{ skipPersistPrevious: true },
		);
	});
});
