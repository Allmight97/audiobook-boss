import type { AcquisitionLane } from '../../types/appSettings';
import type {
	ProviderId,
	RemoteAuthStartResponse,
	RemoteLibraryResponse,
	RemoteSourceAccountState,
	RemoteSourceProviderCapabilities,
} from '../../types/remoteSource';
import type { RemoteSourcePatch } from './types';
import { uniqueDiagnosticMessage } from './display';
import { laneSelectionResetPatch } from './types';
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
}

export type RemoteSourceWorkflowAction =
	| { readonly type: 'refreshAccount' }
	| { readonly type: 'enterLane'; readonly lane: AcquisitionLane }
	| { readonly type: 'startAuth' }
	| { readonly type: 'completeAuth' }
	| { readonly type: 'logout' }
	| { readonly type: 'loadLibrary' };

export type RemoteSourceWorkflow = {
	run(action: RemoteSourceWorkflowAction): Promise<void>;
	invalidate(): void;
};

type WorkflowScope = { readonly isCurrent: () => boolean; readonly providerId: ProviderId };

export function createRemoteSourceWorkflow(deps: {
	readonly services: RemoteSourceWorkflowServices;
	readonly state: RemoteSourceStateStore;
}): RemoteSourceWorkflow {
	let workflowGeneration = 0;
	let entryGeneration = 0;
	function invalidate(): void {
		workflowGeneration += 1;
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
			patchWhenCurrent(scope, {
				titles: library.titles,
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

	async function enterLane(lane: AcquisitionLane, scope: WorkflowScope): Promise<void> {
		const current = deps.state.current();
		const providerId = lane;
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
				!deps.state.current().isAcquiring
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
				// The engine refuses disconnect while accepted work or handoff needs credentials.
				const providerId = deps.state.current().providerId;
				patchWhenCurrent(workflowScope, { isBusy: true });
				try {
					const accountState = await deps.services.logout(providerId);
					patchWhenCurrent(workflowScope, {
						accountState,
						titles: [],
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
			default: {
				const _exhaustive: never = action;
				return _exhaustive;
			}
		}
	}

	return {
		async run(action) {
			const generation = workflowGeneration;
			const entry = entryGeneration;
			const providerId = deps.state.current().providerId;
			const survivesEntry = action.type === 'enterLane';
			const scope: WorkflowScope = {
				providerId,
				isCurrent: () =>
					workflowGeneration === generation && (survivesEntry || entryGeneration === entry),
			};
			await runAction(action, scope);
		},
		invalidate,
	};
}
