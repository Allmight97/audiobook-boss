import { initFileImport } from '../ui/fileImport';
import { displayFileList, selectFile } from '../ui/fileList/actions';
import { initEncoderPanel } from '../ui/encoderPanel';
import { initOutputPanel } from '../ui/outputPanel';
import { updateOutputDirectory, updateNamingPreset } from '../ui/outputPanel/state';
import { initStatusPanel } from '../ui/statusPanel';
import { initCoverArt } from '../ui/coverArt';
import { initMetadataFormEvents } from '../ui/metadataForm';
import { initTagPreview } from '../ui/tagPreview';
import { initMetadataLookup } from '../ui/metadataLookup';
import { initJobControls, setJobTypeSelection } from '../ui/jobControls';
import { clearMetadataState, setMetadataForFile } from '../ui/metadataState';
import {
	HARNESS_FILE_LIST,
	HARNESS_METADATA_BY_FILE,
	HARNESS_OUTPUT_DIRECTORY,
} from './sampleData';

function seedMetadataState(): void {
	clearMetadataState();
	for (const [filePath, metadata] of Object.entries(HARNESS_METADATA_BY_FILE)) {
		setMetadataForFile(filePath, metadata);
	}
}

export async function bootstrapHarnessRuntime(): Promise<void> {
	initFileImport();
	initEncoderPanel();
	initOutputPanel();
	initStatusPanel();
	initCoverArt();
	initMetadataFormEvents();
	initTagPreview();
	initMetadataLookup();
	initJobControls();

	updateOutputDirectory(HARNESS_OUTPUT_DIRECTORY);
	updateNamingPreset('absDefault');
	setJobTypeSelection('batch');

	displayFileList(HARNESS_FILE_LIST);
	seedMetadataState();
	await selectFile(0, { multi: false, range: false }, { skipPersistPrevious: true });
}
