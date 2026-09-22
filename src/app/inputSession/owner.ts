import { createSignal, type Accessor } from 'solid-js';
import type { AudioFile, AudioHandling, ProcessPayload, JobType } from '../../types/audio';
import { liveInputCapability, type InputCapability } from '../../lib/tauri/capabilities/input';
import { toInputView } from './display';
import { runImportIntent } from './importWorkflow';
import {
	replaceFileListFiles,
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
	readonly capability: Accessor<InputCapability>;
	sourcesFor(file: AudioFile): ReadonlyArray<AudioFile>;
	groupSelected(): Promise<void>;
	ungroup(file: AudioFile): Promise<void>;
	reorderSources(file: AudioFile, from: number, to: number): void;
	audioChoiceRequired(file: AudioFile): boolean;
	audioHandling(file: AudioFile): AudioHandling;
	setAudioHandling(file: AudioFile, handling: AudioHandling): void;
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

	function sourcesFor(file: AudioFile): ReadonlyArray<AudioFile> {
		rev();
		return session.titleSourcesByIdentity[fileIdentityKey(file)] ?? [file];
	}

	return {
		sourcesFor,
		audioChoiceRequired(file) {
			rev();
			return session.audioChoiceRequired.includes(fileIdentityKey(file));
		},
		async groupSelected() {
			if (session.orderLocked || session.selectedIndices.length < 2) return;
			const selected = [...session.selectedIndices]
				.sort((a, b) => a - b)
				.map((index) => session.fileList?.files[index])
				.filter((file): file is AudioFile => Boolean(file));
			if (!(await allowSelectionTransition()) || session.orderLocked) return;
			if (selected.some((file) => currentIndex(file) < 0)) return;
			const anchor = selected[0] ? session.fileList?.files[currentIndex(selected[0])] : undefined;
			if (!anchor) return;
			const key = fileIdentityKey(anchor);
			const selectedKeys = new Set(selected.map(fileIdentityKey));
			const sources = selected.flatMap((file) => {
				const current = session.fileList?.files[currentIndex(file)];
				return current ? sourcesFor(current) : [];
			});
			const choices = new Set(
				selected.map((file) => session.audioHandlingByIdentity[fileIdentityKey(file)] ?? 'encode'),
			);
			const files = (session.fileList?.files ?? []).filter(
				(file) => fileIdentityKey(file) === key || !selectedKeys.has(fileIdentityKey(file)),
			);
			const index = files.findIndex((file) => fileIdentityKey(file) === key);
			commit({
				...replaceFileListFiles(session, files),
				titleSourcesByIdentity: { ...session.titleSourcesByIdentity, [key]: sources },
				audioChoiceRequired: [
					...session.audioChoiceRequired.filter((id) => !selectedKeys.has(id)),
					...(choices.size > 1 ||
					selected.some((file) => session.audioChoiceRequired.includes(fileIdentityKey(file)))
						? [key]
						: []),
				],
				selectedIndices: [index],
				selectedAnchor: index,
				sortDirection: 'none',
			});
		},
		async ungroup(file) {
			if (session.orderLocked || !(await allowSelectionTransition())) return;
			const index = currentIndex(file);
			const sources = sourcesFor(file);
			if (index < 0 || sources.length < 2 || session.orderLocked) return;
			const groups = { ...session.titleSourcesByIdentity };
			for (const source of sources) delete groups[fileIdentityKey(source)];
			const files = [...(session.fileList?.files ?? [])];
			files.splice(index, 1, ...sources);
			commit({
				...replaceFileListFiles(session, files),
				titleSourcesByIdentity: groups,
				audioChoiceRequired: session.audioChoiceRequired.filter(
					(id) => id !== fileIdentityKey(file),
				),
				selectedIndices: sources.map((_, offset) => index + offset),
				selectedAnchor: index,
			});
		},
		reorderSources(file, from, to) {
			if (session.orderLocked || currentIndex(file) < 0) return;
			const sources = [...sourcesFor(file)];
			if (from < 0 || to < 0 || from >= sources.length || to >= sources.length || from === to)
				return;
			const [moved] = sources.splice(from, 1);
			if (!moved) return;
			sources.splice(to, 0, moved);
			commit({
				...session,
				titleSourcesByIdentity: {
					...session.titleSourcesByIdentity,
					[fileIdentityKey(file)]: sources,
				},
			});
		},
		view,
		session: sessionView,
		capability,
		audioHandling(file) {
			rev();
			return session.audioHandlingByIdentity[fileIdentityKey(file)] ?? 'encode';
		},
		setAudioHandling(file, handling) {
			if (session.orderLocked) return;
			const current = session.fileList?.files[currentIndex(file)];
			if (
				!current ||
				(handling === 'preserve' &&
					!sourcesFor(current).every((source) => source.preservation?.canPreserve))
			)
				return;
			commit({
				...session,
				audioChoiceRequired: session.audioChoiceRequired.filter(
					(id) => id !== fileIdentityKey(current),
				),
				audioHandlingByIdentity: {
					...session.audioHandlingByIdentity,
					[fileIdentityKey(current)]: handling,
				},
			});
		},
		chooseCue(inputId, choice) {
			if (session.orderLocked) return;
			const current = session;
			if (!current.fileList) return;
			const updateFile = (file: AudioFile): AudioFile => {
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
			};
			const files = current.fileList.files.map(updateFile);
			const titleSourcesByIdentity = Object.fromEntries(
				Object.entries(current.titleSourcesByIdentity).map(([id, sources]) => [
					id,
					sources.map(updateFile),
				]),
			);
			commit({ ...current, titleSourcesByIdentity, fileList: { ...current.fileList, files } });
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
		replaceSession(next) {
			selectionTransition?.abort();
			commit(next);
		},
		reset() {
			selectionTransition?.abort();
			selectionTransition = undefined;
			importEpoch += 1;
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
