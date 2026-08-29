import type { FileListInfo } from '../../types/audio';
import type {
	AcquisitionJob,
	RemoteAuthStartResponse,
	RemoteLibraryResponse,
	RemoteSourceAccountState,
} from '../../types/remoteSource';
import {
	Effect,
	type AppEffect,
	type AppLayer,
	makeWorkflowKit,
	runAppEffect,
} from '../../lib/effect/appEffect';
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
import type { RemoteInputHandoffResult } from './types';

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

export type RemoteSourceWorkflowServicesId = 'RemoteSource/WorkflowServices';
export type RemoteSourceWorkflowLayer = AppLayer<RemoteSourceWorkflowServicesId>;

const kit = makeWorkflowKit(
	'RemoteSource/WorkflowServices',
	'RemoteSourceWorkflowFailed',
)<RemoteSourceWorkflowServices>();

export const RemoteSourceWorkflowServicesTag = kit.Tag;

export function makeRemoteSourceWorkflowServicesLayer(
	services: RemoteSourceWorkflowServices,
): RemoteSourceWorkflowLayer {
	return kit.makeLive(services);
}

export const RemoteSourceWorkflowFailed = kit.Failed;
export type RemoteSourceWorkflowFailed = InstanceType<typeof kit.Failed>;

const workflowPromise = kit.tryPromise;

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

function asProgressJob(job: AcquisitionJob): AcquisitionJobWithProgress {
	return job;
}

async function refreshAccountState(services: RemoteSourceWorkflowServices): Promise<void> {
	const accountState = await services.getAccountState();
	patchRemoteSourceState({ accountState });
}

async function loadLibrary(services: RemoteSourceWorkflowServices): Promise<void> {
	patchRemoteSourceState({ isBusy: true });
	try {
		const library = await services.loadLibrary();
		const selectableTitleIds = new Set(
			library.titles.filter((title) => isTitleAcquirable(title)).map((title) => title.titleId),
		);
		patchRemoteSourceState({
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
		setAcquisitionError(cause, 'Failed to load Audible library.');
	} finally {
		patchRemoteSourceState({ isBusy: false });
	}
}

async function pollAcquisitionToTerminal(
	services: RemoteSourceWorkflowServices,
	initialJob: AcquisitionJobWithProgress,
): Promise<AcquisitionJobWithProgress> {
	let currentJob = initialJob;
	while (!isAcquisitionTerminal(currentJob)) {
		await services.sleep(acquisitionPollDelayMs);
		currentJob = asProgressJob(await services.getAcquisitionStatus(currentJob.jobId));
		patchRemoteSourceState({
			activeJob: currentJob,
			lastJob: currentJob,
			statusMessage: statusFromAcquisitionJob(currentJob),
		});
	}
	return currentJob;
}

function fileListHasPath(fileList: FileListInfo | null, path: string): boolean {
	return Boolean(fileList?.files.some((file) => file.path === path));
}

async function finishAcquisitionJob(
	services: RemoteSourceWorkflowServices,
	job: AcquisitionJobWithProgress,
): Promise<void> {
	const materializedPaths = job.materializedFiles.map((file) => file.path);
	if (materializedPaths.length === 0) {
		patchRemoteSourceState({
			statusMessage:
				uniqueDiagnosticMessage(job.diagnostics) ||
				'Audible acquisition did not materialize an importable file.',
		});
		return;
	}

	const importResult = await services.importMaterializedPaths(materializedPaths);
	if (importResult.status !== 'imported') {
		await services.purgeSession(job.jobId);
		const cleanedJob = withClearedHandoffJob(job);
		patchRemoteSourceState({
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
		const cleanedJob = withClearedHandoffJob(job);
		patchRemoteSourceState({
			activeJob: cleanedJob,
			lastJob: cleanedJob,
			statusMessage: `${importResult.fileList ? 'Acquired titles were not added to the input session.' : 'Input session had no files after import.'} ${STAGED_FILES_REMOVED_SUFFIX}`,
		});
		return;
	}

	registerRemoteSourceSupplementalAssets(job, importResult.fileList);
	patchRemoteSourceState({
		statusMessage: `${materializedPaths.length} acquired title${materializedPaths.length === 1 ? '' : 's'} imported.`,
	});
}

async function runAction(
	services: RemoteSourceWorkflowServices,
	action: RemoteSourceWorkflowAction,
): Promise<void> {
	switch (action.type) {
		case 'hydrateOpenDialog': {
			patchRemoteSourceState({ isBusy: true, didHydrateOpenDialog: true });
			try {
				await refreshAccountState(services);
				if (remoteSourceState.accountState?.status === 'connected') {
					await loadLibrary(services);
				}
			} catch (cause) {
				setAcquisitionError(cause, 'Failed to load remote source state.');
			} finally {
				patchRemoteSourceState({ isBusy: false });
			}
			return;
		}
		case 'startAuth': {
			patchRemoteSourceState({ isBusy: true });
			try {
				const response = await services.startAuth();
				patchRemoteSourceState({ statusMessage: response.message });
				await services.openAuthorizationUrl(response.authorizationUrl);
			} catch (cause) {
				setAcquisitionError(cause, 'Failed to start Audible auth.');
			} finally {
				patchRemoteSourceState({ isBusy: false });
			}
			return;
		}
		case 'completeAuth': {
			patchRemoteSourceState({ isBusy: true });
			try {
				const accountState = await services.completeAuth(
					remoteSourceState.handoffPath.trim() || undefined,
				);
				patchRemoteSourceState({ accountState, statusMessage: 'Audible connected.' });
				await loadLibrary(services);
			} catch (cause) {
				setAcquisitionError(cause, 'Failed to complete Audible auth.');
			} finally {
				patchRemoteSourceState({ isBusy: false });
			}
			return;
		}
		case 'logout': {
			patchRemoteSourceState({ isBusy: true });
			try {
				const accountState = await services.logout();
				patchRemoteSourceState({
					accountState,
					titles: [],
					selectedTitleIds: new Set(),
					includePdfByTitleId: {},
					activeJob: null,
					lastJob: null,
					statusMessage: 'Audible disconnected.',
				});
			} catch (cause) {
				setAcquisitionError(cause, 'Failed to disconnect Audible.');
			} finally {
				patchRemoteSourceState({ isBusy: false });
			}
			return;
		}
		case 'loadLibrary': {
			await loadLibrary(services);
			return;
		}
		case 'acquireSelected': {
			if (remoteSourceState.selectedTitleIds.size === 0) {
				patchRemoteSourceState({ statusMessage: 'Select at least one Audible title.' });
				return;
			}
			patchRemoteSourceState({
				isBusy: true,
				activeJob: null,
				lastJob: null,
				statusMessage: 'Starting Audible acquisition.',
			});
			try {
				const startedJob = asProgressJob(
					await services.startAcquisition(
						[...remoteSourceState.selectedTitleIds].map((titleId) => ({
							titleId,
							includeSupplementalPdf: remoteSourceState.includePdfByTitleId[titleId] ?? false,
						})),
					),
				);
				patchRemoteSourceState({
					activeJob: startedJob,
					lastJob: startedJob,
					statusMessage: statusFromAcquisitionJob(startedJob),
				});
				const terminalJob = await pollAcquisitionToTerminal(services, startedJob);
				await finishAcquisitionJob(services, terminalJob);
			} catch (cause) {
				setAcquisitionError(cause, 'Failed to acquire selected Audible titles.');
			} finally {
				patchRemoteSourceState({ isBusy: false });
			}
			return;
		}
		case 'cancelActiveAcquisition': {
			const activeJob = remoteSourceState.activeJob;
			if (!activeJob || isAcquisitionTerminal(activeJob)) return;
			try {
				const cancelledJob = asProgressJob(await services.cancelAcquisition(activeJob.jobId));
				patchRemoteSourceState({
					activeJob: cancelledJob,
					lastJob: cancelledJob,
					statusMessage: statusFromAcquisitionJob(cancelledJob),
				});
			} catch (cause) {
				setAcquisitionError(cause, 'Failed to cancel Audible acquisition.');
			}
			return;
		}
		default: {
			const _exhaustive: never = action;
			return _exhaustive;
		}
	}
}

export function remoteSourceWorkflowExecution(
	action: RemoteSourceWorkflowAction,
): AppEffect<void, RemoteSourceWorkflowFailed, RemoteSourceWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* RemoteSourceWorkflowServicesTag;
		yield* workflowPromise(() => runAction(services, action), 'Remote source workflow failed.');
	});
}

export async function runRemoteSourceWorkflow(
	layer: RemoteSourceWorkflowLayer,
	action: RemoteSourceWorkflowAction,
	onStateChange?: () => void,
): Promise<void> {
	const unsubscribe = onStateChange ? subscribeRemoteSourceState(onStateChange) : undefined;
	try {
		await runAppEffect(remoteSourceWorkflowExecution(action).pipe(Effect.provide(layer)));
	} finally {
		unsubscribe?.();
		onStateChange?.();
	}
}
