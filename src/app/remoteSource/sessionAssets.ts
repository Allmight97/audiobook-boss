import type { AudioFile, FileListInfo, SupplementalProcessingAsset } from '../../types/audio';
import { logAppError } from '../../lib/tauri/appError';
import type { AcquisitionJob, SupplementalAsset } from '../../types/remoteSource';

type InputId = string | undefined;
type AssetsByInputId = Record<string, SupplementalProcessingAsset[]>;

export type CompanionAssetSummary = {
	text: string;
	title: string;
	pdfCount: number;
	fileCountWithCompanions: number;
};

export type RemoteSourceSessionAssets = {
	register(job: AcquisitionJob, fileList: FileListInfo | null): void;
	processingAssets(inputIds: readonly InputId[]): AssetsByInputId | undefined;
	hasCompanions(inputId: InputId): boolean;
	companionSummary(inputIds: readonly InputId[]): CompanionAssetSummary;
	withSubmissionRetention<T>(inputIds: readonly InputId[], submit: () => Promise<T>): Promise<T>;
	settleTerminalWork(input: {
		readonly inputIds: readonly string[];
		readonly completedInputIds: readonly string[];
	}): Promise<void>;
	reconcileInput(files: readonly Pick<AudioFile, 'inputId'>[]): Promise<void>;
	reset(): void;
};

export function createRemoteSourceSessionAssets(deps: {
	readonly purgeSession: (jobId: string) => Promise<void>;
	readonly onChange: () => void;
}): RemoteSourceSessionAssets {
	let supplementalAssetsByInputId: AssetsByInputId = {};
	let jobIdsByInputId: Record<string, string> = {};
	let retainedInputIdCounts: Record<string, number> = {};
	let pendingPurgeInputIds: Record<string, true> = {};
	let seenInputIds = new Set<string>();

	function assetsForInputId(inputId: InputId): SupplementalProcessingAsset[] {
		if (!inputId) return [];
		return supplementalAssetsByInputId[inputId] ?? [];
	}

	function processingAssets(inputIds: readonly InputId[]): AssetsByInputId | undefined {
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

	function uniqueInputIds(inputIds: readonly InputId[]): string[] {
		return Array.from(new Set(inputIds.filter((inputId): inputId is string => Boolean(inputId))));
	}

	function retain(inputIds: readonly InputId[]): void {
		const next = { ...retainedInputIdCounts };
		for (const inputId of uniqueInputIds(inputIds)) {
			next[inputId] = (next[inputId] ?? 0) + 1;
		}
		retainedInputIdCounts = next;
	}

	function release(inputIds: readonly InputId[]): string[] {
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

	function remove(inputIds: readonly InputId[]): void {
		const nextAssets = { ...supplementalAssetsByInputId };
		const nextJobs = { ...jobIdsByInputId };
		const nextRetained = { ...retainedInputIdCounts };
		const nextPending = { ...pendingPurgeInputIds };
		for (const inputId of inputIds) {
			if (!inputId) continue;
			delete nextAssets[inputId];
			delete nextJobs[inputId];
			delete nextRetained[inputId];
			delete nextPending[inputId];
		}
		supplementalAssetsByInputId = nextAssets;
		jobIdsByInputId = nextJobs;
		retainedInputIdCounts = nextRetained;
		pendingPurgeInputIds = nextPending;
		deps.onChange();
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

	async function purge(inputIds: readonly InputId[]): Promise<void> {
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
				await deps.purgeSession(jobId);
			} catch (cause) {
				logAppError(`Failed to purge remote source session: ${jobId}`, cause, 'warn');
			}
		}
		remove(purgeableIds);
	}

	return {
		register(job, fileList) {
			if (!fileList) return;

			const nextAssets = { ...supplementalAssetsByInputId };
			const nextJobs = { ...jobIdsByInputId };
			for (const materialized of job.materializedFiles) {
				const importedFile = fileList.files.find((file) => file.path === materialized.path);
				if (!importedFile?.inputId) continue;
				nextJobs[importedFile.inputId] = job.jobId;

				const assets = job.supplementalAssets
					.filter((asset) => asset.titleId === materialized.titleId)
					.map((asset) => toProcessingAsset(asset, importedFile.inputId as string));
				if (assets.length > 0) {
					nextAssets[importedFile.inputId] = assets;
				}
			}

			supplementalAssetsByInputId = nextAssets;
			jobIdsByInputId = nextJobs;
			deps.onChange();
		},
		processingAssets,
		hasCompanions: (inputId) => assetsForInputId(inputId).length > 0,
		companionSummary(inputIds) {
			const selectedCount = inputIds.length;
			const assetsByInput = inputIds.map(assetsForInputId);
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
		},
		async withSubmissionRetention(inputIds, submit) {
			retain(inputIds);
			try {
				return await submit();
			} catch (cause) {
				const pendingPurge = release(inputIds);
				await purge(pendingPurge);
				throw cause;
			}
		},
		async settleTerminalWork({ inputIds, completedInputIds }) {
			const pendingPurge = release(inputIds);
			await purge(Array.from(new Set([...completedInputIds, ...pendingPurge])));
		},
		async reconcileInput(files) {
			const current = new Set(uniqueInputIds(files.map((file) => file.inputId)));
			const removed = [...seenInputIds].filter((inputId) => !current.has(inputId));
			seenInputIds = current;
			if (removed.length > 0) {
				await purge(removed);
			}
		},
		reset() {
			supplementalAssetsByInputId = {};
			jobIdsByInputId = {};
			retainedInputIdCounts = {};
			pendingPurgeInputIds = {};
			seenInputIds = new Set();
			deps.onChange();
		},
	};
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
