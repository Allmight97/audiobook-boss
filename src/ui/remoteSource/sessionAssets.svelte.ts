import type { AudioFile, FileListInfo, SupplementalProcessingAsset } from '../../types/audio';
import type { ProcessCommandResult } from '../../types/audio';
import { normalizeAppError } from '../../lib/tauri/appError';
import { tauriClient } from '../../lib/tauri/client';
import type { AcquisitionJob, SupplementalAsset } from '../../types/remoteSource';

let supplementalAssetsByInputId = $state<Record<string, SupplementalProcessingAsset[]>>({});
let jobIdsByInputId = $state<Record<string, string>>({});

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
		nextJobs[importedFile.inputId as string] = job.jobId;

		const assets = job.supplementalAssets
			.filter((asset) => asset.titleId === materialized.titleId)
			.map((asset) => toProcessingAsset(asset, importedFile.inputId as string));
		if (assets.length > 0) {
			next[importedFile.inputId as string] = assets;
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

export function supplementalAssetsForInputId(
	inputId: string | undefined,
): SupplementalProcessingAsset[] {
	if (!inputId) return [];
	return supplementalAssetsByInputId[inputId] ?? [];
}

export function hasSupplementalAssetsForInputId(inputId: string | undefined): boolean {
	return supplementalAssetsForInputId(inputId).length > 0;
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
	for (const inputId of inputIds) {
		if (inputId) {
			delete next[inputId];
			delete nextJobs[inputId];
		}
	}
	supplementalAssetsByInputId = next;
	jobIdsByInputId = nextJobs;
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
	const ids = Array.from(
		new Set(inputIds.filter((inputId): inputId is string => Boolean(inputId))),
	);
	const idsToRemove = new Set(ids);
	const jobIds = Array.from(
		new Set(ids.flatMap((inputId) => jobIdsByInputId[inputId] ?? [])),
	).filter((jobId) => jobIsReadyForSessionPurge(jobId, idsToRemove));
	for (const jobId of jobIds) {
		try {
			await tauriClient.purgeRemoteSourceSession(jobId);
		} catch (cause) {
			const error = normalizeAppError(cause, 'Failed to purge remote source session.');
			console.warn(
				`Failed to purge remote source session: ${jobId} code=${error.code} category=${error.category}`,
			);
		}
	}
	removeRemoteSourceSupplementalAssets(ids);
}

export async function purgeSuccessfulRemoteSourceSessions(
	result: ProcessCommandResult,
	inputIds: readonly (string | undefined)[],
): Promise<void> {
	if (result.jobType !== 'batch') return;

	const successfulInputIds = result.results
		.filter((entry) => entry.status === 'success' && typeof entry.inputIndex === 'number')
		.map((entry) => inputIds[entry.inputIndex as number])
		.filter((inputId): inputId is string => Boolean(inputId));
	await purgeRemoteSourceSessionsForInputIds(successfulInputIds);
}
