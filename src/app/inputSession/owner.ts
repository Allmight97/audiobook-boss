import { createSignal, type Accessor } from 'solid-js';
import type { JobType } from '../../types/audio';
import { liveInputCapability, type InputCapability } from '../../lib/tauri/capabilities/input';
import { toInputView } from './display';
import { runImportIntent } from './importWorkflow';
import {
	clearAllFilesFromSession,
	moveFileInSession,
	removeFileFromSession,
	reorderFilesInSession,
	restoreImportOrderInSession,
	setOrderLockedInSession,
	sortFilesInSession,
} from './order';
import { clearSelectionInSession, selectAllInSession, selectFileInSession } from './selection';
import {
	emptyInputSession,
	type ImportIntent,
	type InputSessionState,
	type InputView,
	type SelectionModifiers,
} from './types';

export type InputOwner = {
	readonly view: Accessor<InputView>;
	readonly session: Accessor<InputSessionState>;
	readonly jobType: Accessor<JobType>;
	readonly capability: Accessor<InputCapability>;
	importIntent(intent: ImportIntent): Promise<void>;
	hydrateSupportText(): Promise<void>;
	selectFile(command: {
		readonly index: number;
		readonly modifiers: SelectionModifiers;
		readonly skipPersistPrevious?: boolean;
	}): Promise<boolean>;
	selectAll(): Promise<void>;
	clearSelection(): Promise<void>;
	setDragOver(isDragOver: boolean): void;
	removeFile(index: number): void;
	clearAllFiles(): Promise<void>;
	moveFile(command: { readonly index: number; readonly direction: 'up' | 'down' }): void;
	reorderFiles(command: { readonly fromIndex: number; readonly toIndex: number }): void;
	toggleSort(): void;
	restoreImportOrder(): void;
	setOrderLocked(orderLocked: boolean): void;
	setJobType(jobType: JobType): void;
	replaceSession(session: InputSessionState): void;
	reset(): void;
};

export type InputOwnerDeps = {
	readonly capability?: InputCapability;
	readonly beforeSelectionChange?: () => boolean | Promise<boolean>;
};

export function createInputOwner(deps: InputOwnerDeps = {}): InputOwner {
	let session = emptyInputSession();
	let jobType: JobType = 'batch';
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	const capabilityValue = deps.capability ?? liveInputCapability;
	const view: Accessor<InputView> = () => {
		rev();
		return toInputView(session);
	};
	const sessionView: Accessor<InputSessionState> = () => {
		rev();
		return session;
	};
	const jobTypeView: Accessor<JobType> = () => {
		rev();
		return jobType;
	};
	const capability: Accessor<InputCapability> = () => capabilityValue;
	let selectionTransitionTicket = 0;
	let importQueue: Promise<void> = Promise.resolve();
	let importEpoch = 0;

	function commit(next: InputSessionState): void {
		session = next;
		bump((n) => n + 1);
	}

	function beginSelectionTransition(): number {
		return ++selectionTransitionTicket;
	}

	function isLatestSelectionTransition(ticket: number): boolean {
		return ticket === selectionTransitionTicket;
	}

	return {
		view,
		session: sessionView,
		jobType: jobTypeView,
		capability,
		async importIntent(intent) {
			const epoch = importEpoch;
			const run = importQueue.then(async () => {
				if (epoch !== importEpoch) {
					return;
				}
				const next = await runImportIntent(capabilityValue, session, intent);
				if (epoch !== importEpoch) {
					return;
				}
				commit({ ...next, orderLocked: session.orderLocked });
			});
			importQueue = run.then(
				() => undefined,
				() => undefined,
			);
			await run;
		},
		async hydrateSupportText() {
			try {
				const metadata = await capabilityValue.getSupportedAudioImportMetadata();
				commit({
					...session,
					supportText: metadata.supportText || session.supportText,
				});
			} catch {}
		},
		async selectFile(command) {
			const ticket = beginSelectionTransition();
			if (!command.skipPersistPrevious && deps.beforeSelectionChange) {
				const allowed = await deps.beforeSelectionChange();
				if (allowed === false) {
					return false;
				}
			}
			if (!isLatestSelectionTransition(ticket)) {
				return false;
			}
			commit(selectFileInSession(session, command.index, command.modifiers));
			return true;
		},
		async selectAll() {
			const ticket = beginSelectionTransition();
			if (deps.beforeSelectionChange) {
				const allowed = await deps.beforeSelectionChange();
				if (allowed === false) {
					return;
				}
			}
			if (!isLatestSelectionTransition(ticket)) {
				return;
			}
			commit(selectAllInSession(session));
		},
		async clearSelection() {
			const ticket = beginSelectionTransition();
			if (deps.beforeSelectionChange) {
				const allowed = await deps.beforeSelectionChange();
				if (allowed === false) {
					return;
				}
			}
			if (!isLatestSelectionTransition(ticket)) {
				return;
			}
			commit(clearSelectionInSession(session));
		},
		setDragOver(isDragOver) {
			if (session.isDragOver === isDragOver) {
				return;
			}
			commit({ ...session, isDragOver });
		},
		removeFile(index) {
			commit(removeFileFromSession(session, index).session);
		},
		async clearAllFiles() {
			const ticket = beginSelectionTransition();
			if (deps.beforeSelectionChange) {
				const allowed = await deps.beforeSelectionChange();
				if (allowed === false) {
					return;
				}
			}
			if (!isLatestSelectionTransition(ticket)) {
				return;
			}
			commit(clearAllFilesFromSession(session));
		},
		moveFile(command) {
			commit(moveFileInSession(session, command.index, command.direction));
		},
		reorderFiles(command) {
			commit(reorderFilesInSession(session, command.fromIndex, command.toIndex));
		},
		toggleSort() {
			commit(sortFilesInSession(session));
		},
		restoreImportOrder() {
			commit(restoreImportOrderInSession(session));
		},
		setOrderLocked(orderLocked) {
			commit(setOrderLockedInSession(session, orderLocked));
		},
		setJobType(next) {
			jobType = next;
			bump((n) => n + 1);
		},
		replaceSession(next) {
			commit(next);
		},
		reset() {
			importEpoch += 1;
			jobType = 'batch';
			commit(emptyInputSession());
		},
	};
}
