import { toUserMessage } from '../../lib/tauri/appError';
import { releaseKey } from './selection';
import type { FileListInfo } from '../../types/audio';
import type { AcquisitionLane } from '../../types/appSettings';
import type {
	AcquisitionJob,
	ProviderId,
	RemoteAuthStartResponse,
	RemoteLibraryResponse,
	RemoteRelease,
	RemoteReleaseGrabRequest,
	RemoteReleaseGrabResponse,
	RemoteReleaseSearchRequest,
	RemoteReleaseSearchResponse,
	RemoteSourceAccountState,
	RemoteSourceProviderCapabilities,
} from '../../types/remoteSource';
import type { RemoteInputHandoffResult, RemoteSourcePatch, RemoteSourceState } from './types';
import {
	acquisitionPollDelayMs,
	isAcquisitionTerminal,
	isTitleAcquirable,
	statusFromAcquisitionJob,
	uniqueDiagnosticMessage,
	withClearedHandoffJob,
	type AcquisitionJobWithProgress,
} from './display';
import { laneSelectionResetPatch, providerIdFromLane } from './types';
import type { RemoteSourceStateStore } from './state';

export interface RemoteSourceWorkflowServices {
	listProviders: () => Promise<RemoteSourceProviderCapabilities[]>;
	getAccountState: (providerId: ProviderId) => Promise<RemoteSourceAccountState>;
	startAuth: (providerId: ProviderId) => Promise<RemoteAuthStartResponse>;
	openAuthorizationUrl: (url: string) => Promise<void>;
	completeAuth: (
		providerId: ProviderId,
		responseUrlHandoffPath?: string,
	) => Promise<RemoteSourceAccountState>;
	logout: (providerId: ProviderId) => Promise<RemoteSourceAccountState>;
	loadLibrary: (providerId: ProviderId) => Promise<RemoteLibraryResponse>;
	searchReleases: (request: RemoteReleaseSearchRequest) => Promise<RemoteReleaseSearchResponse>;
	grabRelease: (request: RemoteReleaseGrabRequest) => Promise<RemoteReleaseGrabResponse>;
	startAcquisition: (
		providerId: ProviderId,
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
	| { readonly type: 'refreshAccount' }
	| { readonly type: 'enterLane'; readonly lane: AcquisitionLane }
	| { readonly type: 'startAuth' }
	| { readonly type: 'completeAuth' }
	| { readonly type: 'logout' }
	| { readonly type: 'loadLibrary' }
	| { readonly type: 'searchReleases' }
	| { readonly type: 'grabSelectedReleases' }
	| { readonly type: 'grabRelease'; readonly release: Pick<RemoteRelease, 'guid' | 'indexerId'> }
	| { readonly type: 'acquireSelected' }
	| { readonly type: 'cancelActiveAcquisition' };

export type RemoteSourceWorkflow = {
	run(action: RemoteSourceWorkflowAction): Promise<void>;
	invalidate(): void;
	clearIndexerResults(): void;
};

export const ORDER_LOCKED_IMPORT_MESSAGE =
	'Order locked while processing. Wait for completion to add files.';

export const STAGED_FILES_REMOVED_SUFFIX =
	'Staged remote files were removed; retry acquisition after processing completes.';

type WorkflowScope = { readonly isCurrent: () => boolean; readonly providerId: ProviderId };

export function createRemoteSourceWorkflow(deps: {
	readonly services: RemoteSourceWorkflowServices;
	readonly state: RemoteSourceStateStore;
	readonly registerSupplementalAssets: (job: AcquisitionJob, fileList: FileListInfo | null) => void;
}): RemoteSourceWorkflow {
	let workflowGeneration = 0;
	let acquisitionGeneration = 0;
	let entryGeneration = 0;
	let indexerConnectionGeneration = 0;

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

	function patchWhenCurrent(scope: WorkflowScope, patch: RemoteSourcePatch): boolean {
		if (!scope.isCurrent()) return false;
		deps.state.patch(patch, scope.providerId);
		return true;
	}

	function setAcquisitionErrorWhenCurrent(
		scope: WorkflowScope,
		cause: unknown,
		fallback: string,
	): void {
		if (scope.isCurrent()) {
			deps.state.setAcquisitionError(cause, fallback, scope.providerId);
		}
	}

	async function refreshAccountState(providerId: ProviderId, scope: WorkflowScope): Promise<void> {
		const accountState = await deps.services.getAccountState(providerId);
		if (deps.state.current().providerId === providerId) {
			patchWhenCurrent(scope, { accountState });
		}
	}

	async function loadLibrary(providerId: ProviderId, scope: WorkflowScope): Promise<void> {
		patchWhenCurrent(scope, { isBusy: true });
		try {
			const library = await deps.services.loadLibrary(providerId);
			if (!scope.isCurrent()) return;
			const selectableTitleIds = new Set(
				library.titles.filter((title) => isTitleAcquirable(title)).map((title) => title.titleId),
			);
			patchWhenCurrent(scope, {
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
			setAcquisitionErrorWhenCurrent(scope, cause, 'Failed to load Audible library.');
		} finally {
			patchWhenCurrent(scope, { isBusy: false });
		}
	}

	async function pollAcquisitionToTerminal(
		initialJob: AcquisitionJobWithProgress,
		scope: WorkflowScope,
	): Promise<AcquisitionJobWithProgress | null> {
		let currentJob = initialJob;
		while (scope.isCurrent() && !isAcquisitionTerminal(currentJob)) {
			await deps.services.sleep(acquisitionPollDelayMs);
			if (!scope.isCurrent()) return null;
			currentJob = await deps.services.getAcquisitionStatus(currentJob.jobId);
			if (
				!patchWhenCurrent(scope, {
					activeJob: currentJob,
					lastJob: currentJob,
					statusMessage: statusFromAcquisitionJob(currentJob),
				})
			) {
				return null;
			}
		}
		return scope.isCurrent() ? currentJob : null;
	}

	async function finishAcquisitionJob(
		job: AcquisitionJobWithProgress,
		scope: WorkflowScope,
	): Promise<void> {
		if (!scope.isCurrent()) return;
		const materializedPaths = job.materializedFiles.map((file) => file.path);
		if (materializedPaths.length === 0) {
			patchWhenCurrent(scope, {
				statusMessage:
					uniqueDiagnosticMessage(job.diagnostics) ||
					'Audible acquisition did not materialize an importable file.',
			});
			return;
		}

		const importResult = await deps.services.importMaterializedPaths(materializedPaths);
		if (!scope.isCurrent()) return;
		if (importResult.status !== 'imported') {
			await deps.services.purgeSession(job.jobId);
			if (!scope.isCurrent()) return;
			const cleanedJob = withClearedHandoffJob(job);
			patchWhenCurrent(scope, {
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
			if (!scope.isCurrent()) return;
			const cleanedJob = withClearedHandoffJob(job);
			patchWhenCurrent(scope, {
				activeJob: cleanedJob,
				lastJob: cleanedJob,
				statusMessage: `${importResult.fileList ? 'Acquired titles were not added to the input session.' : 'Input session had no files after import.'} ${STAGED_FILES_REMOVED_SUFFIX}`,
			});
			return;
		}

		if (!scope.isCurrent()) return;
		deps.registerSupplementalAssets(job, importResult.fileList);
		patchWhenCurrent(scope, {
			statusMessage: `${materializedPaths.length} acquired title${materializedPaths.length === 1 ? '' : 's'} imported.`,
		});
	}

	function setGrabState(
		key: string,
		status: RemoteSourceState['releaseGrabs'][string],
		scope: WorkflowScope,
	): void {
		patchWhenCurrent(scope, {
			releaseGrabs: { ...deps.state.current().releaseGrabs, [key]: status },
		});
	}

	async function sendRelease(release: RemoteRelease, scope: WorkflowScope): Promise<void> {
		const key = releaseKey(release);
		setGrabState(key, { status: 'sending', message: 'Sending to downloader via Indexer…' }, scope);
		try {
			const response = await deps.services.grabRelease({ release });
			setGrabState(
				key,
				{
					status: response.accepted ? 'sent' : 'error',
					message: response.accepted
						? response.message
						: uniqueDiagnosticMessage(response.diagnostics) ||
							response.message ||
							'Indexer did not accept the grab.',
				},
				scope,
			);
		} catch (cause) {
			setGrabState(
				key,
				{
					status: 'error',
					message: toUserMessage(cause, {
						fallback: 'Could not confirm the handoff. Check your downloader before retrying.',
						suppressUnknown: true,
					}),
				},
				scope,
			);
		}
	}

	async function grabReleases(keys: ReadonlySet<string>, scope: WorkflowScope): Promise<void> {
		const current = deps.state.current();
		const releases = current.releases.filter(
			(release) =>
				keys.has(releaseKey(release)) &&
				current.releaseGrabs[releaseKey(release)]?.status !== 'sent',
		);
		if (current.isBusy || current.providerId !== 'indexer' || releases.length === 0) return;
		patchWhenCurrent(scope, {
			isGrabbing: true,
			statusMessage: 'Sending to downloader via Indexer…',
			releaseGrabs: {
				...current.releaseGrabs,
				...Object.fromEntries(
					releases.map((release) => [
						releaseKey(release),
						{ status: 'queued' as const, message: 'Waiting to send.' },
					]),
				),
			},
		});
		try {
			for (const release of releases) {
				if (!scope.isCurrent()) return;
				await sendRelease(release, scope);
			}
			if (!scope.isCurrent()) return;
			const outcomes = releases.map(
				(release) => deps.state.current().releaseGrabs[releaseKey(release)],
			);
			const sent = outcomes.filter((outcome) => outcome.status === 'sent').length;
			patchWhenCurrent(scope, {
				statusMessage:
					outcomes.length === 1
						? outcomes[0].message
						: `${sent} sent to downloader; ${outcomes.length - sent} could not be confirmed. See individual results.`,
			});
		} finally {
			if (scope.isCurrent()) {
				deps.state.patch({ isGrabbing: false });
			}
		}
	}

	async function enterLane(lane: AcquisitionLane, scope: WorkflowScope): Promise<void> {
		const current = deps.state.current();
		const providerId = providerIdFromLane(lane);
		if (current.providerId === providerId && current.isBusy) return;
		if (providerId !== current.providerId) {
			deps.state.patch({
				providerId,
				...laneSelectionResetPatch(),
				accountState: null,
				isBusy: false,
			});
		}
		const generation = ++entryGeneration;
		const entryScope: WorkflowScope = {
			providerId,
			isCurrent: () => scope.isCurrent() && entryGeneration === generation,
		};
		if (providerId !== current.providerId && providerId === 'indexer') {
			patchWhenCurrent(entryScope, { statusMessage: '' });
		}
		patchWhenCurrent(entryScope, { isBusy: true });
		try {
			const providers = await deps.services.listProviders();
			if (!patchWhenCurrent(entryScope, { providers })) return;
			await refreshAccountState(providerId, entryScope);
			if (!entryScope.isCurrent()) return;
			if (
				providerId === 'audible' &&
				deps.state.current().accountState?.status === 'connected' &&
				!deps.state.current().activeJob
			) {
				await loadLibrary(providerId, entryScope);
			}
		} catch (cause) {
			setAcquisitionErrorWhenCurrent(entryScope, cause, 'Failed to load remote source state.');
		} finally {
			patchWhenCurrent(entryScope, { isBusy: false });
		}
	}

	async function runAction(
		action: RemoteSourceWorkflowAction,
		workflowScope: WorkflowScope,
	): Promise<void> {
		switch (action.type) {
			case 'enterLane':
				await enterLane(action.lane, workflowScope);
				return;
			case 'refreshAccount': {
				try {
					await refreshAccountState(deps.state.current().providerId, workflowScope);
				} catch (cause) {
					setAcquisitionErrorWhenCurrent(
						workflowScope,
						cause,
						'Connection saved, but account refresh failed. Reopen Acquire to retry.',
					);
				}
				return;
			}
			case 'startAuth': {
				const providerId = deps.state.current().providerId;
				patchWhenCurrent(workflowScope, { isBusy: true });
				try {
					const response = await deps.services.startAuth(providerId);
					if (!patchWhenCurrent(workflowScope, { statusMessage: response.message })) return;
					await deps.services.openAuthorizationUrl(response.authorizationUrl);
				} catch (cause) {
					setAcquisitionErrorWhenCurrent(workflowScope, cause, 'Failed to start Audible auth.');
				} finally {
					patchWhenCurrent(workflowScope, { isBusy: false });
				}
				return;
			}
			case 'completeAuth': {
				const providerId = deps.state.current().providerId;
				patchWhenCurrent(workflowScope, { isBusy: true });
				try {
					const accountState = await deps.services.completeAuth(
						providerId,
						deps.state.current().handoffPath.trim() || undefined,
					);
					if (
						!patchWhenCurrent(workflowScope, {
							accountState,
							statusMessage: 'Audible connected.',
						})
					) {
						return;
					}
					await loadLibrary(providerId, workflowScope);
				} catch (cause) {
					setAcquisitionErrorWhenCurrent(workflowScope, cause, 'Failed to complete Audible auth.');
				} finally {
					patchWhenCurrent(workflowScope, { isBusy: false });
				}
				return;
			}
			case 'logout': {
				const providerId = deps.state.current().providerId;
				patchWhenCurrent(workflowScope, { isBusy: true });
				try {
					const accountState = await deps.services.logout(providerId);
					patchWhenCurrent(workflowScope, {
						accountState,
						titles: [],
						selectedTitleIds: new Set(),
						includePdfByTitleId: {},
						activeJob: null,
						lastJob: null,
						statusMessage: 'Audible disconnected.',
					});
				} catch (cause) {
					setAcquisitionErrorWhenCurrent(workflowScope, cause, 'Failed to disconnect Audible.');
				} finally {
					patchWhenCurrent(workflowScope, { isBusy: false });
				}
				return;
			}
			case 'loadLibrary': {
				await loadLibrary(deps.state.current().providerId, workflowScope);
				return;
			}
			case 'searchReleases': {
				const author = deps.state.current().indexerAuthorQuery.trim();
				const title = deps.state.current().indexerTitleQuery.trim();
				if (!author && !title) {
					patchWhenCurrent(workflowScope, {
						statusMessage: 'Enter an author and/or title to search.',
					});
					return;
				}
				patchWhenCurrent(workflowScope, {
					isBusy: true,
					selectedReleaseKeys: new Set(),
					releaseGrabs: {},
					releases: [],
					statusMessage: 'Searching Indexer releases.',
				});
				try {
					const response = await deps.services.searchReleases({
						author: author || undefined,
						title: title || undefined,
						query: undefined,
					});
					if (!workflowScope.isCurrent()) return;
					patchWhenCurrent(workflowScope, {
						releases: response.releases,
						statusMessage:
							response.diagnostics.length > 0
								? uniqueDiagnosticMessage(response.diagnostics)
								: `${response.releases.length} release${response.releases.length === 1 ? '' : 's'} found.`,
					});
				} catch (cause) {
					setAcquisitionErrorWhenCurrent(
						workflowScope,
						cause,
						'Failed to search Indexer releases.',
					);
				} finally {
					patchWhenCurrent(workflowScope, { isBusy: false });
				}
				return;
			}
			case 'grabSelectedReleases': {
				await grabReleases(deps.state.current().selectedReleaseKeys, workflowScope);
				return;
			}
			case 'grabRelease': {
				await grabReleases(new Set([releaseKey(action.release)]), workflowScope);
				return;
			}
			case 'acquireSelected': {
				if (deps.state.current().isAcquiring) return;
				const generation = beginAcquisition();
				const acquisitionScope: WorkflowScope = {
					providerId: workflowScope.providerId,
					isCurrent: () => workflowScope.isCurrent() && acquisitionGeneration === generation,
				};
				const current = deps.state.current();
				if (current.selectedTitleIds.size === 0) {
					patchWhenCurrent(acquisitionScope, {
						statusMessage: 'Select at least one Audible title.',
					});
					return;
				}
				patchWhenCurrent(acquisitionScope, {
					isAcquiring: true,
					activeJob: null,
					lastJob: null,
					statusMessage: 'Starting Audible acquisition.',
				});
				try {
					const selections = [...current.selectedTitleIds].map((titleId) => ({
						titleId,
						includeSupplementalPdf: current.includePdfByTitleId[titleId] ?? false,
					}));
					const startedJob = await deps.services.startAcquisition(current.providerId, selections);
					if (
						!patchWhenCurrent(acquisitionScope, {
							activeJob: startedJob,
							lastJob: startedJob,
							statusMessage: statusFromAcquisitionJob(startedJob),
						})
					) {
						return;
					}
					const terminalJob = await pollAcquisitionToTerminal(startedJob, acquisitionScope);
					if (terminalJob) {
						await finishAcquisitionJob(terminalJob, acquisitionScope);
					}
				} catch (cause) {
					setAcquisitionErrorWhenCurrent(
						acquisitionScope,
						cause,
						'Failed to acquire selected Audible titles.',
					);
				} finally {
					patchWhenCurrent(acquisitionScope, { isAcquiring: false });
				}
				return;
			}
			case 'cancelActiveAcquisition': {
				const activeJob = deps.state.current().activeJob;
				if (!activeJob || isAcquisitionTerminal(activeJob)) return;
				try {
					const cancelledJob = await deps.services.cancelAcquisition(activeJob.jobId);
					if (!workflowScope.isCurrent()) return;
					invalidateAcquisition();
					patchWhenCurrent(workflowScope, {
						activeJob: cancelledJob,
						lastJob: cancelledJob,
						statusMessage: statusFromAcquisitionJob(cancelledJob),
						isAcquiring: false,
					});
				} catch (cause) {
					setAcquisitionErrorWhenCurrent(
						workflowScope,
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
			if (deps.state.current().isGrabbing) return;
			const generation = workflowGeneration;
			const entry = entryGeneration;
			const connection = indexerConnectionGeneration;
			const providerId = deps.state.current().providerId;
			const survivesEntry =
				action.type === 'enterLane' ||
				action.type === 'acquireSelected' ||
				action.type === 'cancelActiveAcquisition';
			const scope: WorkflowScope = {
				providerId,
				isCurrent: () =>
					workflowGeneration === generation &&
					(survivesEntry ||
						(entryGeneration === entry &&
							(providerId !== 'indexer' || indexerConnectionGeneration === connection))),
			};
			await runAction(action, scope);
		},
		invalidate,
		clearIndexerResults() {
			indexerConnectionGeneration += 1;
			deps.state.patch(
				{
					releases: [],
					selectedReleaseKeys: new Set(),
					releaseGrabs: {},
					isBusy: false,
					statusMessage: 'Search again before grabbing releases with the saved Indexer connection.',
				},
				'indexer',
			);
		},
	};
}

function fileListHasPath(fileList: FileListInfo | null, path: string): boolean {
	return Boolean(fileList?.files.some((file) => file.path === path));
}
