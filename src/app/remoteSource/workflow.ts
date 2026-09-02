import type { FileListInfo } from '../../types/audio';
import type {
	AcquisitionJob,
	RemoteAuthStartResponse,
	RemoteLibraryResponse,
	RemoteSourceAccountState,
} from '../../types/remoteSource';
import type { RemoteInputHandoffResult, RemoteSourceState } from './types';
import {
	acquisitionPollDelayMs,
	isAcquisitionTerminal,
	isTitleAcquirable,
	statusFromAcquisitionJob,
	uniqueDiagnosticMessage,
	withClearedHandoffJob,
	type AcquisitionJobWithProgress,
} from './display';
import type { RemoteSourceStateStore } from './state';

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

export type RemoteSourceWorkflow = {
	run(action: RemoteSourceWorkflowAction): Promise<void>;
	invalidate(): void;
};

export const ORDER_LOCKED_IMPORT_MESSAGE =
	'Order locked while processing. Wait for completion to add files.';

export const STAGED_FILES_REMOVED_SUFFIX =
	'Staged remote files were removed; retry acquisition after processing completes.';

type IsCurrent = () => boolean;

export function createRemoteSourceWorkflow(deps: {
	readonly services: RemoteSourceWorkflowServices;
	readonly state: RemoteSourceStateStore;
	readonly registerSupplementalAssets: (job: AcquisitionJob, fileList: FileListInfo | null) => void;
}): RemoteSourceWorkflow {
	let workflowGeneration = 0;
	let acquisitionGeneration = 0;

	function invalidate(): void {
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

	function patchWhenCurrent(isCurrent: IsCurrent, patch: Partial<RemoteSourceState>): boolean {
		if (!isCurrent()) return false;
		deps.state.patch(patch);
		return true;
	}

	function setAcquisitionErrorWhenCurrent(
		isCurrent: IsCurrent,
		cause: unknown,
		fallback: string,
	): void {
		if (isCurrent()) {
			deps.state.setAcquisitionError(cause, fallback);
		}
	}

	async function refreshAccountState(isCurrent: IsCurrent): Promise<void> {
		const accountState = await deps.services.getAccountState();
		patchWhenCurrent(isCurrent, { accountState });
	}

	async function loadLibrary(isCurrent: IsCurrent): Promise<void> {
		patchWhenCurrent(isCurrent, { isBusy: true });
		try {
			const library = await deps.services.loadLibrary();
			if (!isCurrent()) return;
			const selectableTitleIds = new Set(
				library.titles.filter((title) => isTitleAcquirable(title)).map((title) => title.titleId),
			);
			patchWhenCurrent(isCurrent, {
				titles: library.titles,
				selectedTitleIds: new Set(
					[...deps.state.current().selectedTitleIds].filter((titleId) =>
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
		initialJob: AcquisitionJobWithProgress,
		isCurrent: IsCurrent,
	): Promise<AcquisitionJobWithProgress | null> {
		let currentJob = initialJob;
		while (isCurrent() && !isAcquisitionTerminal(currentJob)) {
			await deps.services.sleep(acquisitionPollDelayMs);
			if (!isCurrent()) return null;
			currentJob = await deps.services.getAcquisitionStatus(currentJob.jobId);
			if (
				!patchWhenCurrent(isCurrent, {
					activeJob: currentJob,
					lastJob: currentJob,
					statusMessage: statusFromAcquisitionJob(currentJob),
				})
			) {
				return null;
			}
		}
		return isCurrent() ? currentJob : null;
	}

	async function finishAcquisitionJob(
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

		const importResult = await deps.services.importMaterializedPaths(materializedPaths);
		if (!isCurrent()) return;
		if (importResult.status !== 'imported') {
			await deps.services.purgeSession(job.jobId);
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
			await deps.services.purgeSession(job.jobId);
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
		deps.registerSupplementalAssets(job, importResult.fileList);
		patchWhenCurrent(isCurrent, {
			statusMessage: `${materializedPaths.length} acquired title${materializedPaths.length === 1 ? '' : 's'} imported.`,
		});
	}

	async function runAction(
		action: RemoteSourceWorkflowAction,
		isWorkflowCurrent: IsCurrent,
	): Promise<void> {
		switch (action.type) {
			case 'hydrateOpenDialog': {
				patchWhenCurrent(isWorkflowCurrent, { isBusy: true, didHydrateOpenDialog: true });
				try {
					await refreshAccountState(isWorkflowCurrent);
					if (!isWorkflowCurrent()) return;
					if (deps.state.current().accountState?.status === 'connected') {
						await loadLibrary(isWorkflowCurrent);
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
					const response = await deps.services.startAuth();
					if (!patchWhenCurrent(isWorkflowCurrent, { statusMessage: response.message })) return;
					await deps.services.openAuthorizationUrl(response.authorizationUrl);
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
					const accountState = await deps.services.completeAuth(
						deps.state.current().handoffPath.trim() || undefined,
					);
					if (
						!patchWhenCurrent(isWorkflowCurrent, {
							accountState,
							statusMessage: 'Audible connected.',
						})
					) {
						return;
					}
					await loadLibrary(isWorkflowCurrent);
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
					const accountState = await deps.services.logout();
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
				await loadLibrary(isWorkflowCurrent);
				return;
			}
			case 'acquireSelected': {
				const generation = beginAcquisition();
				const isAcquisitionCurrent = () =>
					isWorkflowCurrent() && acquisitionGeneration === generation;
				const current = deps.state.current();
				if (current.selectedTitleIds.size === 0) {
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
					const selections = [...current.selectedTitleIds].map((titleId) => ({
						titleId,
						includeSupplementalPdf: current.includePdfByTitleId[titleId] ?? false,
					}));
					const startedJob = await deps.services.startAcquisition(selections);
					if (
						!patchWhenCurrent(isAcquisitionCurrent, {
							activeJob: startedJob,
							lastJob: startedJob,
							statusMessage: statusFromAcquisitionJob(startedJob),
						})
					) {
						return;
					}
					const terminalJob = await pollAcquisitionToTerminal(startedJob, isAcquisitionCurrent);
					if (terminalJob) {
						await finishAcquisitionJob(terminalJob, isAcquisitionCurrent);
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
				const activeJob = deps.state.current().activeJob;
				if (!activeJob || isAcquisitionTerminal(activeJob)) return;
				try {
					const cancelledJob = await deps.services.cancelAcquisition(activeJob.jobId);
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

	return {
		async run(action) {
			const generation = workflowGeneration;
			const isCurrent = () => workflowGeneration === generation;
			await runAction(action, isCurrent);
		},
		invalidate,
	};
}

function fileListHasPath(fileList: FileListInfo | null, path: string): boolean {
	return Boolean(fileList?.files.some((file) => file.path === path));
}
