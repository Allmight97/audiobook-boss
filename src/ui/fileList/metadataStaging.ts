import { tauriClient } from '../../lib/tauri/client';
import type { AudioFile } from '../../types/audio';
import { applyMetadataDraftIntent, hasActionableMetadataDraftIntent } from '../metadataDraft';
import {
	firstMetadataIntentValidationError,
	validateMetadataDraftIntent,
} from '../metadataValidation';
import { hasDirtyMetadataFields, readMetadataForm, resetDirtyState } from '../metadataForm';
import { getMetadataForFile, metadataEqualsNullish, setMetadataForFile } from '../metadataState';
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
	const validation = await validateMetadataDraftIntent(
		metadata,
		tauriClient.validateMetadataIntentPatch,
	);
	const validationError = firstMetadataIntentValidationError(validation.result);
	if (validationError) {
		setStatusMessage(validationError);
		return false;
	}

	const existing = getMetadataForFile(file.path) ?? {};
	const intentPatch = validation.intentPatch;
	if (!hasActionableMetadataDraftIntent(intentPatch)) {
		return true;
	}
	const merged = applyMetadataDraftIntent(existing, intentPatch);
	if (metadataEqualsNullish(existing, merged)) {
		return true;
	}

	setMetadataForFile(file.path, merged, {
		markPending: true,
		intentPatch,
	});
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

	const validation = await validateMetadataDraftIntent(
		changes,
		tauriClient.validateMetadataIntentPatch,
	);
	const validationError = firstMetadataIntentValidationError(validation.result);
	if (validationError) {
		if (options?.showStatus) {
			setStatusMessage(validationError);
		}
		return false;
	}

	await ensureMetadataForFiles(selectedFiles);
	const intentPatch = validation.intentPatch;
	if (!hasActionableMetadataDraftIntent(intentPatch)) {
		return true;
	}

	selectedFiles.forEach((file) => {
		const existing = getMetadataForFile(file.path) ?? {};
		const merged = applyMetadataDraftIntent(existing, intentPatch);
		if (!metadataEqualsNullish(existing, merged)) {
			setMetadataForFile(file.path, merged, {
				markPending: true,
				intentPatch,
			});
		}
	});

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
