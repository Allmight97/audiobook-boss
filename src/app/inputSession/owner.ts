import { createMemo, createSignal, type Accessor } from 'solid-js';
import type { AudioFile, ProcessPayload, JobType } from '../../types/audio';
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
	chooseCue(inputId: string, choice: 'confirmHundredths' | 'ignore'): void;
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
	let selectionTransitionTicket = 0;

	function commit(next: InputSessionState): void {
		setSession(next);
	}

	function beginSelectionTransition(): number {
		return ++selectionTransitionTicket;
	}

	function isLatestSelectionTransition(ticket: number): boolean {
		return ticket === selectionTransitionTicket;
	}

	return {
		view,
		session,
		jobType,
		capability,
		chooseCue(inputId, choice) {
			if (session().orderLocked) return;
			const current = session();
			if (!current.fileList) return;
			const files = current.fileList.files.map((file) => {
				if (file.inputId !== inputId || !file.cueSource) return file;
				if (choice === 'confirmHundredths' && file.cueSource.status === 'needsConfirmation') {
					return { ...file, cueSource: { ...file.cueSource, status: 'ready' as const } };
				}
				if (choice === 'ignore' && file.cueSource.status !== 'embeddedPreferred') {
					return {
						...file,
						cueSource: { ...file.cueSource, status: 'ignored' as const },
						chapterPlan: file.chapterPlan
							? { ...file.chapterPlan, fromCue: false, chapters: file.chapters ?? [] }
							: undefined,
					};
				}
				return file;
			});
			commit({ ...current, fileList: { ...current.fileList, files } });
		},
		async importIntent(intent) {
			const next = await runAppEffect(runImportIntent(capability(), session(), intent));
			commit({ ...next, orderLocked: session().orderLocked });
		},
		async hydrateSupportText() {
			try {
				const metadata = await capability().getSupportedAudioImportMetadata();
				commit({
					...session(),
					supportText: metadata.supportText || session().supportText,
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
			commit(selectFileInSession(session(), command.index, command.modifiers));
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
			commit(selectAllInSession(session()));
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

export function chapterPlansForProcessing(
	files: readonly AudioFile[],
	jobType: JobType,
): ProcessPayload['chapterPlans'] {
	const plans: NonNullable<ProcessPayload['chapterPlans']> = {};
	for (const file of files.filter((file) => file.isValid)) {
		if (file.cueSource?.status === 'needsConfirmation' || file.cueSource?.status === 'invalid') {
			throw new Error(
				`Review ${file.cueSource.fileName}: confirm its timestamp interpretation or ignore the CUE before converting.`,
			);
		}
		if (jobType === 'merge' && files.length > 1 && file.chapterPlan?.fromCue) {
			throw new Error(
				'Merging CUE-bearing inputs is not supported. Convert separate jobs or ignore CUE chapters.',
			);
		}
		if (file.chapterPlan)
			plans[file.path] = {
				...file.chapterPlan,
				chapters: file.chapterPlan.chapters.map((chapter) => ({
					...chapter,
					title: chapter.title,
				})),
			};
	}
	return Object.keys(plans).length ? plans : undefined;
}
