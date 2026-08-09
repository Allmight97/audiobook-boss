import { tauriClient } from '../../lib/tauri/client';
import type { AudioFile } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import { hasDirtyMetadataFields, readMetadataForm, resetDirtyState } from '../metadataForm';
import {
	cacheMetadataForFile,
	getMetadataForFile,
	stageMetadataIntentPatch,
	validateMetadataDraft,
} from '../metadataSession';
import { updateEstimatedSize, updateOutputPath } from '../outputPanel';
import { pushStatusPanelTransientStatus } from '../statusPanel';
import { getSelectedFiles } from './metadataPanel';
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

export type PreparedMetadataDraft =
	| { kind: 'none' }
	| {
			kind: 'single';
			filePath: string;
			intentPatch: Parameters<typeof stageMetadataIntentPatch>[1];
	  }
	| {
			kind: 'multi';
			files: AudioFile[];
			intentPatch: Parameters<typeof stageMetadataIntentPatch>[1];
			pendingCacheByPath: Record<string, Partial<AudiobookMetadata>>;
	  };

export type PrepareMetadataDraftsResult =
	| { ok: true; prepared: PreparedMetadataDraft }
	| { ok: false };

async function readMetadataSnapshot(file: AudioFile): Promise<Partial<AudiobookMetadata> | null> {
	if (!file.isValid) return null;
	const existing = getMetadataForFile(file.path);
	const hasUsableCache = Boolean(
		existing && Object.keys(existing).some((key) => key !== 'cover_art'),
	);
	if (hasUsableCache) return null;
	try {
		return await tauriClient.readAudioMetadata(file.path);
	} catch (error) {
		console.warn('Failed to load metadata:', error);
		return null;
	}
}

async function prepareSingleSelectionMetadata(
	file: AudioFile | null,
): Promise<PrepareMetadataDraftsResult> {
	if (!file?.isValid || !hasDirtyMetadataFields()) return { ok: true, prepared: { kind: 'none' } };
	const metadata = readMetadataForm({ mode: 'single' });
	const validation = await validateMetadataDraft(metadata, tauriClient.validateMetadataIntentPatch);
	if (!validation.ok) {
		setStatusMessage(validation.errors.first ?? 'Metadata validation failed.');
		return { ok: false };
	}
	return {
		ok: true,
		prepared: { kind: 'single', filePath: file.path, intentPatch: validation.intentPatch },
	};
}

async function prepareMultiSelectionMetadata(options?: {
	showStatus?: boolean;
	selectedFilesOverride?: AudioFile[];
}): Promise<PrepareMetadataDraftsResult> {
	if (!getCurrentFileList()) return { ok: true, prepared: { kind: 'none' } };
	const selectedFiles = (options?.selectedFilesOverride ?? getSelectedFiles()).filter(
		(file) => file.isValid,
	);
	if (selectedFiles.length === 0) return { ok: true, prepared: { kind: 'none' } };
	const changes = readMetadataForm({ mode: 'multi', onlyDirty: true });
	if (Object.keys(changes).length === 0) {
		if (options?.showStatus) setStatusMessage('No metadata changes to apply');
		return { ok: true, prepared: { kind: 'none' } };
	}
	const validation = await validateMetadataDraft(changes, tauriClient.validateMetadataIntentPatch);
	if (!validation.ok) {
		if (options?.showStatus)
			setStatusMessage(validation.errors.first ?? 'Metadata validation failed.');
		return { ok: false };
	}
	const pendingCacheByPath: Record<string, Partial<AudiobookMetadata>> = {};
	await Promise.all(
		selectedFiles.map(async (file) => {
			const snapshot = await readMetadataSnapshot(file);
			if (snapshot) pendingCacheByPath[file.path] = snapshot;
		}),
	);
	return {
		ok: true,
		prepared: {
			kind: 'multi',
			files: selectedFiles,
			intentPatch: validation.intentPatch,
			pendingCacheByPath,
		},
	};
}

/** Synchronous commit: callers must revalidate list/lock immediately before
 * and after calling this function. */
export function commitPreparedMetadataDrafts(prepared: PreparedMetadataDraft): boolean {
	if (prepared.kind === 'none') return true;
	if (prepared.kind === 'single') {
		if (stageMetadataIntentPatch(prepared.filePath, prepared.intentPatch) !== 'staged') return true;
		resetDirtyState();
		refreshOutputForMetadataChange();
		return true;
	}
	for (const [path, metadata] of Object.entries(prepared.pendingCacheByPath)) {
		const existing = getMetadataForFile(path);
		const hasUsableCache = Boolean(
			existing && Object.keys(existing).some((key) => key !== 'cover_art'),
		);
		if (!hasUsableCache) cacheMetadataForFile(path, metadata);
	}
	const stageResults = prepared.files.map((file) =>
		stageMetadataIntentPatch(file.path, prepared.intentPatch),
	);
	if (stageResults[0] === 'noop') return true;
	resetDirtyState();
	refreshOutputForMetadataChange();
	return true;
}

export async function prepareMetadataDraftsForCurrentSelection(options?: {
	validationFailureMessage?: string;
}): Promise<PrepareMetadataDraftsResult> {
	const selectedFiles = getSelectedFiles().filter((file) => file.isValid);
	if (selectedFiles.length === 0) return { ok: true, prepared: { kind: 'none' } };
	const prepared =
		selectedFiles.length === 1
			? await prepareSingleSelectionMetadata(selectedFiles[0])
			: await prepareMultiSelectionMetadata();
	if (!prepared.ok && options?.validationFailureMessage)
		setStatusMessage(options.validationFailureMessage);
	return prepared;
}

export async function persistSingleSelectionMetadata(file: AudioFile | null): Promise<boolean> {
	const prepared = await prepareSingleSelectionMetadata(file);
	return prepared.ok && commitPreparedMetadataDrafts(prepared.prepared);
}

export async function stageMetadataToSelection(options?: {
	showStatus?: boolean;
	selectedFilesOverride?: AudioFile[];
}): Promise<boolean> {
	const prepared = await prepareMultiSelectionMetadata(options);
	if (!prepared.ok) return false;
	const committed = commitPreparedMetadataDrafts(prepared.prepared);
	if (committed && prepared.prepared.kind === 'multi' && options?.showStatus) {
		setTransientStatusMessage(`Draft saved for ${prepared.prepared.files.length} files`);
	}
	return committed;
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
	if (selectedFiles.length === 0 || (options?.skipSingleSelection && selectedFiles.length === 1))
		return true;
	const prepared = await prepareMetadataDraftsForCurrentSelection({
		validationFailureMessage: options?.validationFailureMessage,
	});
	return prepared.ok && commitPreparedMetadataDrafts(prepared.prepared);
}
