import type { FileListInfo } from '../../types/audio';
import type {
	AcquisitionJob,
	RemoteAuthStartResponse,
	RemoteLibraryResponse,
	RemoteSourceAccountState,
} from '../../types/remoteSource';
import type { RemoteInputHandoffResult } from './types';
import {
	acquisitionPollDelayMs,
	isAcquisitionTerminal,
	isTitleAcquirable,
	statusFromAcquisitionJob,
	uniqueDiagnosticMessage,
	withClearedHandoffJob,
	type AcquisitionJobWithProgress,
} from './display';
import { registerRemoteSourceSupplementalAssets } from './sessionAssets';
import {
	patchRemoteSourceState,
	remoteSourceState,
	setAcquisitionError,
	subscribeRemoteSourceState,
} from './state';

export interface RemoteSourceWorkflowServices {
	getAccountState: () => Promise<RemoteSourceAccountState>;
	startAuth: () => Promise<RemoteAuthStartResponse>;
	openAuthorizationUrl: (url: string) => Promise<void>;
	completeAuth: (responseUrlHandoffPath?: string) => Promise<RemoteSourceAccountState>;
	logout: () => Promise<RemoteSourceAccountState>;
	loadLibrary: () => Promise<RemoteLibraryResponse>;
	startAcquisition: (
		selections: ReadonlyArray<{
			readonly titleId: string;
			readonly includeSupplementalPdf: boolean;
		}>,
	) => Promise<AcquisitionJob>;
	getAcquisitionStatus: (jobId: string) => Promise<AcquisitionJob>;
	cancelAcquisition: (jobId: string) => Promise<AcquisitionJob>;
	purgeSession: (jobId: string) => Promise<void>;
	importMaterializedPaths: (paths: readonly string[]) => Promise<RemoteInputHandoffResult>;
	sleep: (ms: number) => Promise<void>;
}

export type RemoteSourceWorkflowAction =
	| { readonly type: 'hydrateOpenDialog' }
	| { readonly type: 'startAuth' }
	| { readonly type: 'completeAuth' }
	| { readonly type: 'logout' }
	| { readonly type: 'loadLibrary' }
	| { readonly type: 'acquireSelected' }
	| { readonly type: 'cancelActiveAcquisition' };

export const ORDER_LOCKED_IMPORT_MESSAGE =
	'Order locked while processing. Wait for completion to add files.';

export const STAGED_FILES_REMOVED_SUFFIX =
	'Staged remote files were removed; retry acquisition after processing completes.';

let workflowGeneration = 0;
let acquisitionGeneration = 0;

type IsCurrent = () => boolean;

export function invalidateRemoteSourceWorkflows(): void {
	workflowGeneration += 1;
	acquisitionGeneration += 1;
}

function beginAcquisition(): number {
	acquisitionGeneration += 1;
	return acquisitionGeneration;
}

function invalidateAcquisition(): void {
	acquisitionGeneration += 1;
}

function patchWhenCurrent(
	isCurrent: IsCurrent,
	patch: Parameters<typeof patchRemoteSourceState>[0],
): boolean {
	if (!isCurrent()) return false;
	patchRemoteSourceState(patch);
	return true;
}

function setAcquisitionErrorWhenCurrent(
	isCurrent: IsCurrent,
	cause: unknown,
	fallback: string,
): void {
	if (isCurrent()) {
		setAcquisitionError(cause, fallback);
	}
}

function asProgressJob(job: AcquisitionJob): AcquisitionJobWithProgress {
	return job;
}

async function refreshAccountState(
	services: RemoteSourceWorkflowServices,
	isCurrent: IsCurrent,
): Promise<void> {
	const accountState = await services.getAccountState();
	patchWhenCurrent(isCurrent, { accountState });
}

async function loadLibrary(
	services: RemoteSourceWorkflowServices,
	isCurrent: IsCurrent,
): Promise<void> {
	patchWhenCurrent(isCurrent, { isBusy: true });
	try {
		const library = await services.loadLibrary();
		if (!isCurrent()) return;
		const selectableTitleIds = new Set(
			library.titles.filter((title) => isTitleAcquirable(title)).map((title) => title.titleId),
		);
		patchWhenCurrent(isCurrent, {
			titles: library.titles,
			selectedTitleIds: new Set(
				[...remoteSourceState.selectedTitleIds].filter((titleId) =>
					selectableTitleIds.has(titleId),
				),
			),
			includePdfByTitleId: Object.fromEntries(
				library.titles.map((title) => [title.titleId, title.supplementalPdfAvailable]),
			),
			statusMessage:
				library.diagnostics.length > 0
					? uniqueDiagnosticMessage(library.diagnostics)
					: `${library.titles.length} Audible titles loaded.`,
		});
	} catch (cause) {
		setAcquisitionErrorWhenCurrent(isCurrent, cause, 'Failed to load Audible library.');
	} finally {
		patchWhenCurrent(isCurrent, { isBusy: false });
	}
}

async function pollAcquisitionToTerminal(
	services: RemoteSourceWorkflowServices,
	initialJob: AcquisitionJobWithProgress,
	isCurrent: IsCurrent,
): Promise<AcquisitionJobWithProgress | null> {
	let currentJob = initialJob;
	while (isCurrent() && !isAcquisitionTerminal(currentJob)) {
		await services.sleep(acquisitionPollDelayMs);
		if (!isCurrent()) return null;
		currentJob = asProgressJob(await services.getAcquisitionStatus(currentJob.jobId));
		if (
			!patchWhenCurrent(isCurrent, {
				activeJob: currentJob,
				lastJob: currentJob,
				statusMessage: statusFromAcquisitionJob(currentJob),
			})
		)
			return null;
	}
	return isCurrent() ? currentJob : null;
}

function fileListHasPath(fileList: FileListInfo | null, path: string): boolean {
	return Boolean(fileList?.files.some((file) => file.path === path));
}

async function finishAcquisitionJob(
	services: RemoteSourceWorkflowServices,
	job: AcquisitionJobWithProgress,
	isCurrent: IsCurrent,
): Promise<void> {
	if (!isCurrent()) return;
	const materializedPaths = job.materializedFiles.map((file) => file.path);
	if (materializedPaths.length === 0) {
		patchWhenCurrent(isCurrent, {
			statusMessage:
				uniqueDiagnosticMessage(job.diagnostics) ||
				'Audible acquisition did not materialize an importable file.',
		});
		return;
	}

	const importResult = await services.importMaterializedPaths(materializedPaths);
	if (!isCurrent()) return;
	if (importResult.status !== 'imported') {
		await services.purgeSession(job.jobId);
		if (!isCurrent()) return;
		const cleanedJob = withClearedHandoffJob(job);
		patchWhenCurrent(isCurrent, {
			activeJob: cleanedJob,
			lastJob: cleanedJob,
			statusMessage: `${importResult.message} ${STAGED_FILES_REMOVED_SUFFIX}`,
		});
		return;
	}

	const importedAny = materializedPaths.some((path) =>
		fileListHasPath(importResult.fileList, path),
	);
	if (!importedAny) {
		await services.purgeSession(job.jobId);
		if (!isCurrent()) return;
		const cleanedJob = withClearedHandoffJob(job);
		patchWhenCurrent(isCurrent, {
			activeJob: cleanedJob,
			lastJob: cleanedJob,
			statusMessage: `${importResult.fileList ? 'Acquired titles were not added to the input session.' : 'Input session had no files after import.'} ${STAGED_FILES_REMOVED_SUFFIX}`,
		});
		return;
	}

	if (!isCurrent()) return;
	registerRemoteSourceSupplementalAssets(job, importResult.fileList);
	patchWhenCurrent(isCurrent, {
		statusMessage: `${materializedPaths.length} acquired title${materializedPaths.length === 1 ? '' : 's'} imported.`,
	});
}

async function runAction(
	services: RemoteSourceWorkflowServices,
	action: RemoteSourceWorkflowAction,
	isWorkflowCurrent: IsCurrent,
): Promise<void> {
	switch (action.type) {
		case 'hydrateOpenDialog': {
			patchWhenCurrent(isWorkflowCurrent, { isBusy: true, didHydrateOpenDialog: true });
			try {
				await refreshAccountState(services, isWorkflowCurrent);
				if (!isWorkflowCurrent()) return;
				if (remoteSourceState.accountState?.status === 'connected') {
					await loadLibrary(services, isWorkflowCurrent);
				}
			} catch (cause) {
				setAcquisitionErrorWhenCurrent(
					isWorkflowCurrent,
					cause,
					'Failed to load remote source state.',
				);
			} finally {
				patchWhenCurrent(isWorkflowCurrent, { isBusy: false });
			}
			return;
		}
		case 'startAuth': {
			patchWhenCurrent(isWorkflowCurrent, { isBusy: true });
			try {
				const response = await services.startAuth();
				if (!patchWhenCurrent(isWorkflowCurrent, { statusMessage: response.message })) return;
				await services.openAuthorizationUrl(response.authorizationUrl);
			} catch (cause) {
				setAcquisitionErrorWhenCurrent(isWorkflowCurrent, cause, 'Failed to start Audible auth.');
			} finally {
				patchWhenCurrent(isWorkflowCurrent, { isBusy: false });
			}
			return;
		}
		case 'completeAuth': {
			patchWhenCurrent(isWorkflowCurrent, { isBusy: true });
			try {
				const accountState = await services.completeAuth(
					remoteSourceState.handoffPath.trim() || undefined,
				);
				if (
					!patchWhenCurrent(isWorkflowCurrent, {
						accountState,
						statusMessage: 'Audible connected.',
					})
				)
					return;
				await loadLibrary(services, isWorkflowCurrent);
			} catch (cause) {
				setAcquisitionErrorWhenCurrent(
					isWorkflowCurrent,
					cause,
					'Failed to complete Audible auth.',
				);
			} finally {
				patchWhenCurrent(isWorkflowCurrent, { isBusy: false });
			}
			return;
		}
		case 'logout': {
			patchWhenCurrent(isWorkflowCurrent, { isBusy: true });
			try {
				const accountState = await services.logout();
				patchWhenCurrent(isWorkflowCurrent, {
					accountState,
					titles: [],
					selectedTitleIds: new Set(),
					includePdfByTitleId: {},
					activeJob: null,
					lastJob: null,
					statusMessage: 'Audible disconnected.',
				});
			} catch (cause) {
				setAcquisitionErrorWhenCurrent(isWorkflowCurrent, cause, 'Failed to disconnect Audible.');
			} finally {
				patchWhenCurrent(isWorkflowCurrent, { isBusy: false });
			}
			return;
		}
		case 'loadLibrary': {
			await loadLibrary(services, isWorkflowCurrent);
			return;
		}
		case 'acquireSelected': {
			const generation = beginAcquisition();
			const isAcquisitionCurrent = () =>
				isWorkflowCurrent() && acquisitionGeneration === generation;
			if (remoteSourceState.selectedTitleIds.size === 0) {
				patchWhenCurrent(isAcquisitionCurrent, {
					statusMessage: 'Select at least one Audible title.',
				});
				return;
			}
			patchWhenCurrent(isAcquisitionCurrent, {
				isBusy: true,
				activeJob: null,
				lastJob: null,
				statusMessage: 'Starting Audible acquisition.',
			});
			try {
				const selections = [...remoteSourceState.selectedTitleIds].map((titleId) => ({
					titleId,
					includeSupplementalPdf: remoteSourceState.includePdfByTitleId[titleId] ?? false,
				}));
				const startedJob = asProgressJob(await services.startAcquisition(selections));
				if (
					!patchWhenCurrent(isAcquisitionCurrent, {
						activeJob: startedJob,
						lastJob: startedJob,
						statusMessage: statusFromAcquisitionJob(startedJob),
					})
				)
					return;
				const terminalJob = await pollAcquisitionToTerminal(
					services,
					startedJob,
					isAcquisitionCurrent,
				);
				if (terminalJob) {
					await finishAcquisitionJob(services, terminalJob, isAcquisitionCurrent);
				}
			} catch (cause) {
				setAcquisitionErrorWhenCurrent(
					isAcquisitionCurrent,
					cause,
					'Failed to acquire selected Audible titles.',
				);
			} finally {
				patchWhenCurrent(isAcquisitionCurrent, { isBusy: false });
			}
			return;
		}
		case 'cancelActiveAcquisition': {
			const activeJob = remoteSourceState.activeJob;
			if (!activeJob || isAcquisitionTerminal(activeJob)) return;
			try {
				const cancelledJob = asProgressJob(await services.cancelAcquisition(activeJob.jobId));
				if (!isWorkflowCurrent()) return;
				invalidateAcquisition();
				patchWhenCurrent(isWorkflowCurrent, {
					activeJob: cancelledJob,
					lastJob: cancelledJob,
					statusMessage: statusFromAcquisitionJob(cancelledJob),
					isBusy: false,
				});
			} catch (cause) {
				setAcquisitionErrorWhenCurrent(
					isWorkflowCurrent,
					cause,
					'Failed to cancel Audible acquisition.',
				);
			}
			return;
		}
		default: {
			const _exhaustive: never = action;
			return _exhaustive;
		}
	}
}

export async function runRemoteSourceWorkflow(
	services: RemoteSourceWorkflowServices,
	action: RemoteSourceWorkflowAction,
	onStateChange?: () => void,
): Promise<void> {
	const generation = workflowGeneration;
	const isCurrent = () => workflowGeneration === generation;
	const unsubscribe = onStateChange ? subscribeRemoteSourceState(onStateChange) : undefined;
	try {
		await runAction(services, action, isCurrent);
	} finally {
		unsubscribe?.();
		if (isCurrent()) {
			onStateChange?.();
		}
	}
}
