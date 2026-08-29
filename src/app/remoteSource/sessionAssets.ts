import type { AudioFile, FileListInfo, SupplementalProcessingAsset } from '../../types/audio';
import { logAppError } from '../../lib/tauri/appError';
import { tauriClient } from '../../lib/tauri/client';
import type { AcquisitionJob, SupplementalAsset } from '../../types/remoteSource';

let supplementalAssetsByInputId: Record<string, SupplementalProcessingAsset[]> = {};
let jobIdsByInputId: Record<string, string> = {};
let retainedInputIdCounts: Record<string, number> = {};
let pendingPurgeInputIds: Record<string, true> = {};
let seenInputIds = new Set<string>();

export type CompanionAssetSummary = {
	text: string;
	title: string;
	pdfCount: number;
	fileCountWithCompanions: number;
};

function findImportedFileByPath(files: readonly AudioFile[], path: string): AudioFile | undefined {
	return files.find((file) => file.path === path);
}

function toProcessingAsset(asset: SupplementalAsset, inputId: string): SupplementalProcessingAsset {
	return {
		assetId: asset.assetId,
		inputId,
		titleId: asset.titleId,
		path: asset.path,
		fileName: asset.fileName,
		sizeBytes: asset.sizeBytes,
		sha256: asset.sha256,
	};
}

export function registerRemoteSourceSupplementalAssets(
	job: AcquisitionJob,
	fileList: FileListInfo | null,
): void {
	if (!fileList) return;

	const next = { ...supplementalAssetsByInputId };
	const nextJobs = { ...jobIdsByInputId };
	for (const materialized of job.materializedFiles) {
		const importedFile = findImportedFileByPath(fileList.files, materialized.path);
		if (!importedFile?.inputId) continue;
		nextJobs[importedFile.inputId] = job.jobId;

		const assets = job.supplementalAssets
			.filter((asset) => asset.titleId === materialized.titleId)
			.map((asset) => toProcessingAsset(asset, importedFile.inputId as string));
		if (assets.length > 0) {
			next[importedFile.inputId] = assets;
		}
	}

	supplementalAssetsByInputId = next;
	jobIdsByInputId = nextJobs;
}

export function supplementalAssetsForInputIds(
	inputIds: readonly (string | undefined)[],
): Record<string, SupplementalProcessingAsset[]> | undefined {
	const selected = Object.fromEntries(
		inputIds
			.filter((inputId): inputId is string => typeof inputId === 'string' && inputId.length > 0)
			.flatMap((inputId) => {
				const assets = supplementalAssetsByInputId[inputId];
				return assets && assets.length > 0 ? [[inputId, assets] as const] : [];
			}),
	);
	return Object.keys(selected).length > 0 ? selected : undefined;
}

export function supplementalAssetsByInputIdForProcessing(
	inputIds: readonly (string | undefined)[],
): Record<string, SupplementalProcessingAsset[]> | undefined {
	return supplementalAssetsForInputIds(inputIds);
}

export function supplementalAssetsForInputId(
	inputId: string | undefined,
): SupplementalProcessingAsset[] {
	if (!inputId) return [];
	return supplementalAssetsByInputId[inputId] ?? [];
}

export function hasSupplementalAssetsForInputId(inputId: string | undefined): boolean {
	return supplementalAssetsForInputId(inputId).length > 0;
}

function uniqueInputIds(inputIds: readonly (string | undefined)[]): string[] {
	return Array.from(new Set(inputIds.filter((inputId): inputId is string => Boolean(inputId))));
}

export function retainRemoteSourceSessionsForInputIds(
	inputIds: readonly (string | undefined)[],
): void {
	const next = { ...retainedInputIdCounts };
	for (const inputId of uniqueInputIds(inputIds)) {
		next[inputId] = (next[inputId] ?? 0) + 1;
	}
	retainedInputIdCounts = next;
}

export function releaseRemoteSourceSessionRetainers(
	inputIds: readonly (string | undefined)[],
): string[] {
	const next = { ...retainedInputIdCounts };
	const pendingToPurge: string[] = [];
	const nextPending = { ...pendingPurgeInputIds };

	for (const inputId of uniqueInputIds(inputIds)) {
		const count = (next[inputId] ?? 0) - 1;
		if (count > 0) {
			next[inputId] = count;
			continue;
		}
		delete next[inputId];
		if (nextPending[inputId]) {
			pendingToPurge.push(inputId);
			delete nextPending[inputId];
		}
	}

	retainedInputIdCounts = next;
	pendingPurgeInputIds = nextPending;
	return pendingToPurge;
}

export function companionSummaryForInputIds(
	inputIds: readonly (string | undefined)[],
): CompanionAssetSummary {
	const selectedCount = inputIds.length;
	const assetsByInput = inputIds.map((inputId) => supplementalAssetsForInputId(inputId));
	const pdfCount = assetsByInput.reduce((count, assets) => count + assets.length, 0);
	const fileCountWithCompanions = assetsByInput.filter((assets) => assets.length > 0).length;
	const title = Array.from(
		new Set(assetsByInput.flatMap((assets) => assets.map((asset) => asset.fileName))),
	).join(', ');

	if (selectedCount === 0) {
		return { text: '---', title, pdfCount, fileCountWithCompanions };
	}
	if (pdfCount === 0) {
		return { text: 'None', title, pdfCount, fileCountWithCompanions };
	}
	if (selectedCount === 1) {
		return {
			text: pdfCount === 1 ? title || 'PDF attached' : `${pdfCount} PDFs attached`,
			title,
			pdfCount,
			fileCountWithCompanions,
		};
	}

	const text = `${pdfCount} ${pdfCount === 1 ? 'PDF' : 'PDFs'} across ${selectedCount} selected files`;
	return {
		text,
		title: text,
		pdfCount,
		fileCountWithCompanions,
	};
}

export function removeRemoteSourceSupplementalAssets(
	inputIds: readonly (string | undefined)[],
): void {
	const next = { ...supplementalAssetsByInputId };
	const nextJobs = { ...jobIdsByInputId };
	const nextRetained = { ...retainedInputIdCounts };
	const nextPending = { ...pendingPurgeInputIds };
	for (const inputId of inputIds) {
		if (inputId) {
			delete next[inputId];
			delete nextJobs[inputId];
			delete nextRetained[inputId];
			delete nextPending[inputId];
		}
	}
	supplementalAssetsByInputId = next;
	jobIdsByInputId = nextJobs;
	retainedInputIdCounts = nextRetained;
	pendingPurgeInputIds = nextPending;
}

function registeredInputIdsForJob(jobId: string): string[] {
	return Object.entries(jobIdsByInputId)
		.filter(([, registeredJobId]) => registeredJobId === jobId)
		.map(([inputId]) => inputId);
}

function jobIsReadyForSessionPurge(jobId: string, inputIdsToRemove: Set<string>): boolean {
	const registeredInputIds = registeredInputIdsForJob(jobId);
	return (
		registeredInputIds.length > 0 &&
		registeredInputIds.every((inputId) => inputIdsToRemove.has(inputId))
	);
}

export async function purgeRemoteSourceSessionsForInputIds(
	inputIds: readonly (string | undefined)[],
): Promise<void> {
	const ids = uniqueInputIds(inputIds);
	const purgeableIds: string[] = [];
	const nextPending = { ...pendingPurgeInputIds };
	for (const inputId of ids) {
		if ((retainedInputIdCounts[inputId] ?? 0) > 0) {
			nextPending[inputId] = true;
		} else {
			purgeableIds.push(inputId);
		}
	}
	pendingPurgeInputIds = nextPending;
	if (purgeableIds.length === 0) return;

	const purgeableIdsToRemove = new Set(purgeableIds);
	const jobIds = Array.from(
		new Set(purgeableIds.flatMap((inputId) => jobIdsByInputId[inputId] ?? [])),
	).filter((jobId) => jobIsReadyForSessionPurge(jobId, purgeableIdsToRemove));
	for (const jobId of jobIds) {
		try {
			await tauriClient.purgeRemoteSourceSession(jobId);
		} catch (cause) {
			logAppError(`Failed to purge remote source session: ${jobId}`, cause, 'warn');
		}
	}
	removeRemoteSourceSupplementalAssets(purgeableIds);
}

export async function reconcileRemoteSourceSessionsWithInput(
	files: readonly Pick<AudioFile, 'inputId'>[],
): Promise<void> {
	const current = new Set(uniqueInputIds(files.map((file) => file.inputId)));
	const removed = [...seenInputIds].filter((inputId) => !current.has(inputId));
	seenInputIds = current;
	if (removed.length > 0) {
		await purgeRemoteSourceSessionsForInputIds(removed);
	}
}

export function resetRemoteSourceSessionAssets(): void {
	supplementalAssetsByInputId = {};
	jobIdsByInputId = {};
	retainedInputIdCounts = {};
	pendingPurgeInputIds = {};
	seenInputIds = new Set();
}
