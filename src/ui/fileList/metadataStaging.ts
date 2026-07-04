import { tauriClient } from '../../lib/tauri/client';
import type { AudioFile } from '../../types/audio';
import { hasDirtyMetadataFields, readMetadataForm, resetDirtyState } from '../metadataForm';
import { stageMetadataIntentPatch, validateMetadataDraft } from '../metadataSession';
import { updateEstimatedSize, updateOutputPath } from '../outputPanel';
import { pushStatusPanelTransientStatus } from '../statusPanel';
import { ensureMetadataForFiles, getSelectedFiles } from './metadataPanel';
import { getCurrentFileList } from './state.svelte';

function refreshOutputForMetadataChange(): void {
	updateOutputPath('final');
	updateEstimatedSize();
}

function setStatusMessage(message: string): void {
	pushStatusPanelTransientStatus(message, { ttlMs: 2_500 });
}

function setTransientStatusMessage(message: string, timeoutMs: number = 2000): void {
	pushStatusPanelTransientStatus(message, { ttlMs: timeoutMs });
}

export async function persistSingleSelectionMetadata(file: AudioFile | null): Promise<boolean> {
	if (!file?.isValid) return true;
	if (!hasDirtyMetadataFields()) return true;

	const metadata = readMetadataForm({ mode: 'single' });
	const validation = await validateMetadataDraft(metadata, tauriClient.validateMetadataIntentPatch);
	if (!validation.ok) {
		setStatusMessage(validation.errors.first ?? 'Metadata validation failed.');
		return false;
	}

	if (stageMetadataIntentPatch(file.path, validation.intentPatch) !== 'staged') {
		return true;
	}
	resetDirtyState();
	refreshOutputForMetadataChange();
	return true;
}

export async function stageMetadataToSelection(options?: {
	showStatus?: boolean;
	selectedFilesOverride?: AudioFile[];
}): Promise<boolean> {
	if (!getCurrentFileList()) return true;

	const selectedFiles = (options?.selectedFilesOverride ?? getSelectedFiles()).filter(
		(file) => file.isValid,
	);
	if (selectedFiles.length === 0) return true;

	const changes = readMetadataForm({ mode: 'multi', onlyDirty: true });
	if (Object.keys(changes).length === 0) {
		if (options?.showStatus) {
			setStatusMessage('No metadata changes to apply');
		}
		return true;
	}

	const validation = await validateMetadataDraft(changes, tauriClient.validateMetadataIntentPatch);
	if (!validation.ok) {
		if (options?.showStatus) {
			setStatusMessage(validation.errors.first ?? 'Metadata validation failed.');
		}
		return false;
	}

	await ensureMetadataForFiles(selectedFiles);
	const stageResults = selectedFiles.map((file) =>
		stageMetadataIntentPatch(file.path, validation.intentPatch),
	);
	// 'noop' is patch-level: the normalized patch carried no actionable ops,
	// so nothing was staged anywhere — keep the form dirty state untouched.
	if (stageResults[0] === 'noop') {
		return true;
	}

	resetDirtyState();
	refreshOutputForMetadataChange();

	if (options?.showStatus) {
		setTransientStatusMessage(`Draft saved for ${selectedFiles.length} files`);
	}

	return true;
}

export async function persistPendingMetadataDraftsForCurrentSelection(options?: {
	showStatus?: boolean;
}): Promise<boolean> {
	const selectedFiles = getSelectedFiles().filter((file) => file.isValid);
	if (selectedFiles.length === 0) {
		return true;
	}
	if (selectedFiles.length === 1) {
		const hadDirtyMetadata = hasDirtyMetadataFields();
		const persisted = await persistSingleSelectionMetadata(selectedFiles[0]);
		if (persisted && hadDirtyMetadata && options?.showStatus) {
			setStatusMessage('Draft saved');
		}
		return persisted;
	}

	return stageMetadataToSelection({ showStatus: options?.showStatus });
}

export async function preserveMetadataDraftsBeforeSelectionChange(options?: {
	skipSingleSelection?: boolean;
	validationFailureMessage?: string;
}): Promise<boolean> {
	const selectedFiles = getSelectedFiles().filter((file) => file.isValid);
	if (selectedFiles.length === 0) {
		return true;
	}
	if (options?.skipSingleSelection && selectedFiles.length === 1) {
		return true;
	}

	const preserved = await persistPendingMetadataDraftsForCurrentSelection({ showStatus: false });
	if (!preserved && selectedFiles.length > 1 && options?.validationFailureMessage) {
		setStatusMessage(options.validationFailureMessage);
	}
	return preserved;
}
