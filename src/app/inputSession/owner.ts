import { createMemo, createSignal, type Accessor } from 'solid-js';
import type { JobType } from '../../types/audio';
import { runAppEffect } from '../../lib/effect/appEffect';
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
	const [session, setSession] = createSignal(emptyInputSession());
	const [jobType, setJobTypeSignal] = createSignal<JobType>('batch');
	const [capability] = createSignal(deps.capability ?? liveInputCapability);
	const view = createMemo(() => toInputView(session()));

	function commit(next: InputSessionState): void {
		setSession(next);
	}

	return {
		view,
		session,
		jobType,
		capability,
		async importIntent(intent) {
			const next = await runAppEffect(runImportIntent(capability(), session(), intent));
			commit({ ...next, orderLocked: session().orderLocked });
		},
		async hydrateSupportText() {
			const current = session();
			try {
				const metadata = await capability().getSupportedAudioImportMetadata();
				commit({
					...session(),
					supportText: metadata.supportText || current.supportText,
				});
			} catch {
				commit(current);
			}
		},
		async selectFile(command) {
			if (!command.skipPersistPrevious && deps.beforeSelectionChange) {
				const allowed = await deps.beforeSelectionChange();
				if (allowed === false) {
					return false;
				}
			}
			commit(selectFileInSession(session(), command.index, command.modifiers));
			return true;
		},
		async selectAll() {
			if (deps.beforeSelectionChange) {
				const allowed = await deps.beforeSelectionChange();
				if (allowed === false) {
					return;
				}
			}
			commit(selectAllInSession(session()));
		},
		async clearSelection() {
			if (deps.beforeSelectionChange) {
				const allowed = await deps.beforeSelectionChange();
				if (allowed === false) {
					return;
				}
			}
			commit(clearSelectionInSession(session()));
		},
		setDragOver(isDragOver) {
			const current = session();
			if (current.isDragOver === isDragOver) {
				return;
			}
			commit({ ...current, isDragOver });
		},
		removeFile(index) {
			commit(removeFileFromSession(session(), index).session);
		},
		async clearAllFiles() {
			if (deps.beforeSelectionChange) {
				const allowed = await deps.beforeSelectionChange();
				if (allowed === false) {
					return;
				}
			}
			commit(clearAllFilesFromSession(session()));
		},
		moveFile(command) {
			commit(moveFileInSession(session(), command.index, command.direction));
		},
		reorderFiles(command) {
			commit(reorderFilesInSession(session(), command.fromIndex, command.toIndex));
		},
		toggleSort() {
			commit(sortFilesInSession(session()));
		},
		restoreImportOrder() {
			commit(restoreImportOrderInSession(session()));
		},
		setOrderLocked(orderLocked) {
			commit(setOrderLockedInSession(session(), orderLocked));
		},
		setJobType(next) {
			setJobTypeSignal(next);
		},
		replaceSession(next) {
			commit(next);
		},
		reset() {
			commit(emptyInputSession());
			setJobTypeSignal('batch');
		},
	};
}
