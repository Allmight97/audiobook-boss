import type { FileListInfo, SupportedAudioImportMetadata } from '../../types/audio';
import type { ImportAnalysisWorkflowServices } from './importAnalysisWorkflow';

export function importOrderLockedMessage(): string {
	return 'Order locked while processing. Wait for completion to add files.';
}

export function unsupportedImportMessage(metadata: SupportedAudioImportMetadata): string {
	return `No supported audio files found. Please use ${metadata.formatsText} files.`;
}

export function reportAnalysisFailure(
	services: ImportAnalysisWorkflowServices,
	cause: unknown,
): FileListInfo | null {
	services.console.error('Failed to analyze files:', cause);
	services.setFileImportError('Failed to analyze files. Please try again.');
	return null;
}

export function reportMetadataStagingFailure(
	services: ImportAnalysisWorkflowServices,
	cause: unknown,
): false {
	services.console.error('Failed to stage metadata drafts:', cause);
	services.setFileImportError(
		'Failed to prepare metadata drafts before adding files. Please try again.',
	);
	return false;
}

export function reportOpenFileDialogFailure(
	services: ImportAnalysisWorkflowServices,
	cause: unknown,
): null {
	services.console.error('Failed to open file dialog:', cause);
	services.setFileImportError('Failed to open file dialog. Please try again.');
	return null;
}

export function reportOpenFolderDialogFailure(
	services: ImportAnalysisWorkflowServices,
	cause: unknown,
): null {
	services.console.error('Failed to open folder dialog:', cause);
	services.setFileImportError('Failed to open folder dialog. Please try again.');
	return null;
}

export function reportImportDiscoveryFailure(
	services: ImportAnalysisWorkflowServices,
	cause: unknown,
): string[] | null {
	services.console.error('Failed to discover audio files:', cause);
	services.setFileImportError('Failed to discover audio files. Please try again.');
	return null;
}

export function reportImportMetadataFailure(
	services: ImportAnalysisWorkflowServices,
	cause: unknown,
): SupportedAudioImportMetadata | null {
	services.console.error('Failed to load supported audio import metadata:', cause);
	services.setFileImportError('Failed to load supported audio formats. Please try again.');
	return null;
}

export function duplicateOnlyImportMessage(): string {
	return 'No new files added. All analyzed files were already in the list.';
}
