import { createSignal, type Accessor } from 'solid-js';
import type { AudioFile, ProcessPayload, JobType } from '../../types/audio';
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
	fileIdentityKey,
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
		readonly signal?: AbortSignal;
	}): Promise<boolean>;
	selectAll(): Promise<void>;
	clearSelection(): Promise<void>;
	setDragOver(isDragOver: boolean): void;
	removeFile(index: number): Promise<void>;
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
	readonly beforeSelectionChange?: (signal?: AbortSignal) => boolean | Promise<boolean>;
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
	let selectionTransition: AbortController | undefined;
	let importQueue: Promise<void> = Promise.resolve();
	let importEpoch = 0;

	function commit(next: InputSessionState): void {
		session = next;
		bump((n) => n + 1);
	}

	async function allowSelectionTransition(signal?: AbortSignal): Promise<boolean> {
		if (signal?.aborted) return false;
		selectionTransition?.abort();
		const transition = new AbortController();
		selectionTransition = transition;
		const abort = () => transition.abort();
		signal?.addEventListener('abort', abort, { once: true });
		try {
			const allowed = await deps.beforeSelectionChange?.(transition.signal);
			return allowed !== false && !transition.signal.aborted;
		} finally {
			signal?.removeEventListener('abort', abort);
		}
	}

	function currentIndex(file: AudioFile): number {
		return (
			session.fileList?.files.findIndex(
				(current) => fileIdentityKey(current) === fileIdentityKey(file),
			) ?? -1
		);
	}

	return {
		view,
		session: sessionView,
		jobType: jobTypeView,
		capability,
		chooseCue(inputId, choice) {
			if (session.orderLocked) return;
			const current = session;
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
			const epoch = importEpoch;
			const run = importQueue.then(async () => {
				if (epoch !== importEpoch) {
					return;
				}
				const applyImport = await runImportIntent(capabilityValue, session, intent);
				if (epoch !== importEpoch) {
					return;
				}
				commit(applyImport(session));
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
			const file = session.fileList?.files[command.index];
			if (!file || !(await allowSelectionTransition(command.signal))) return false;
			const index = currentIndex(file);
			if (index < 0) return false;
			commit(selectFileInSession(session, index, command.modifiers));
			return true;
		},
		async selectAll() {
			if (!(await allowSelectionTransition())) return;
			commit(selectAllInSession(session));
		},
		async clearSelection() {
			if (!(await allowSelectionTransition())) return;
			commit(clearSelectionInSession(session));
		},
		setDragOver(isDragOver) {
			if (session.isDragOver === isDragOver) {
				return;
			}
			commit({ ...session, isDragOver });
		},
		async removeFile(index) {
			const file = session.fileList?.files[index];
			if (!file || session.orderLocked || !(await allowSelectionTransition())) return;
			commit(removeFileFromSession(session, currentIndex(file)).session);
		},
		async clearAllFiles() {
			if (!(await allowSelectionTransition())) return;
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
			selectionTransition?.abort();
			commit(next);
		},
		reset() {
			selectionTransition?.abort();
			selectionTransition = undefined;
			importEpoch += 1;
			jobType = 'batch';
			commit(emptyInputSession());
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
