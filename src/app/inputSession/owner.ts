import { createSignal, type Accessor } from 'solid-js';
import type { AudioFile, ProcessPayload, TitleAudioRequest } from '../../types/audio';
import { toUserMessage } from '../../lib/tauri/appError';
import { liveInputCapability, type InputCapability } from '../../lib/tauri/capabilities/input';
import type { SessionIntent } from '../../types/session';
import type { EngineLink } from '../engineLink';
import { toInputView } from './display';
import {
	DEFAULT_SUPPORT_TEXT,
	fileIdentityKey,
	type ImportIntent,
	type InputView,
	type SelectionModifiers,
} from './types';

/**
 * The engine owns the titles, their order, and the selection. This owner
 * shows them and turns what the user does into intents.
 */
export type InputOwner = {
	audioRequest(file: AudioFile): TitleAudioRequest | undefined;
	setAudioRequest(file: AudioFile, request: TitleAudioRequest): void;
	readonly view: Accessor<InputView>;
	readonly capability: Accessor<InputCapability>;
	sourcesFor(file: AudioFile): ReadonlyArray<AudioFile>;
	groupSelected(): Promise<void>;
	ungroup(file: AudioFile): Promise<void>;
	reorderSources(file: AudioFile, from: number, to: number): void;
	audioChoiceRequired(file: AudioFile): boolean;
	importIntent(intent: ImportIntent): Promise<void>;
	hydrateSupportText(): Promise<void>;
	/** Resolves true when the selection changed and its tags have loaded. */
	selectFile(command: {
		readonly index: number;
		readonly modifiers: SelectionModifiers;
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
	reset(): void;
};

export type InputOwnerDeps = {
	readonly link: EngineLink;
	readonly capability?: InputCapability;
	/** The audio request a title takes as it enters the list. */
	readonly audioDefaults: () => TitleAudioRequest;
	readonly beforeImport?: () => Promise<void>;
};

export function createInputOwner(deps: InputOwnerDeps): InputOwner {
	const { link } = deps;
	const capabilityValue = deps.capability ?? liveInputCapability;
	const capability: Accessor<InputCapability> = () => capabilityValue;
	const [rev, bump] = createSignal(0, { ownedWrite: true });
	// View-local state: none of it is session truth.
	let localError = '';
	let isDragOver = false;
	let supportText = DEFAULT_SUPPORT_TEXT;
	// A title's audio request as just chosen, shown until the engine confirms
	// it. It goes when encoding moves into the engine.
	const chosenAudio = new Map<string, { readonly request: TitleAudioRequest }>();
	// Readers compare requests by identity to decide whether to re-plan, so a
	// request keeps one object while its content stays the same.
	const lastAudio = new Map<string, { content: string; request: TitleAudioRequest }>();

	function sameObjectWhileUnchanged(
		id: string,
		request: TitleAudioRequest | undefined,
	): TitleAudioRequest | undefined {
		if (!request) {
			lastAudio.delete(id);
			return undefined;
		}
		const content = JSON.stringify(request);
		const last = lastAudio.get(id);
		if (last?.content === content) return last.request;
		lastAudio.set(id, { content, request });
		return request;
	}

	function changed(): void {
		bump((n) => n + 1);
	}

	const view: Accessor<InputView> = () => {
		rev();
		return toInputView(link.titles(), link.selection(), {
			errorMessage: localError,
			isDragOver,
			supportText,
		});
	};

	function sourcesFor(file: AudioFile): ReadonlyArray<AudioFile> {
		return link.titles().titleSourcesByIdentity[fileIdentityKey(file)] ?? [file];
	}

	function isLoaded(file: AudioFile): boolean {
		const id = fileIdentityKey(file);
		return link.titles().files.some((current) => fileIdentityKey(current) === id);
	}

	async function applied(intent: SessionIntent): Promise<boolean> {
		return (await link.send(intent)).kind === 'applied';
	}

	async function importPaths(paths: ReadonlyArray<string>): Promise<void> {
		await link.send({ kind: 'import', paths: [...paths], defaultAudio: deps.audioDefaults() });
	}

	async function pick<A>(
		open: () => Promise<A | null>,
		fallback: string,
	): Promise<A | null | undefined> {
		try {
			return await open();
		} catch (cause) {
			localError = toUserMessage(cause, { fallback, suppressUnknown: true });
			changed();
			return undefined;
		}
	}

	async function runImport(intent: ImportIntent): Promise<void> {
		switch (intent.type) {
			case 'pickFiles': {
				const selected = await pick(async () => {
					const supported = await capabilityValue.getSupportedAudioImportMetadata();
					return capabilityValue.openFiles({
						filters: [{ name: 'Audio Files', extensions: [...supported.extensions] }],
					});
				}, 'Failed to open file dialog. Please try again.');
				if (selected?.length) await importPaths(selected);
				return;
			}
			case 'pickFolder': {
				const selected = await pick(
					() => capabilityValue.openDirectory(),
					'Failed to open folder dialog. Please try again.',
				);
				if (selected) await importPaths([selected]);
				return;
			}
			case 'drainOpened':
				await link.send({ kind: 'importOpened', defaultAudio: deps.audioDefaults() });
				return;
			case 'importPaths':
				await importPaths(intent.paths);
		}
	}

	return {
		audioRequest(file) {
			rev();
			const id = fileIdentityKey(file);
			return sameObjectWhileUnchanged(
				id,
				chosenAudio.get(id)?.request ?? link.titles().audioRequestsByIdentity[id],
			);
		},
		setAudioRequest(file, request) {
			if (link.titles().orderLocked || !isLoaded(file)) return;
			const id = fileIdentityKey(file);
			const chosen = { request: structuredClone(request) };
			chosenAudio.set(id, chosen);
			changed();
			link
				.send({ kind: 'setAudioRequest', titleId: id, request: chosen.request })
				.catch((error: unknown) => console.error('Failed to set the audio request:', error))
				.finally(() => {
					if (chosenAudio.get(id) !== chosen) return;
					chosenAudio.delete(id);
					changed();
				});
		},
		sourcesFor,
		audioChoiceRequired(file) {
			rev();
			const id = fileIdentityKey(file);
			return !chosenAudio.has(id) && link.titles().audioChoiceRequired.includes(id);
		},
		async groupSelected() {
			await link.send({ kind: 'groupSelected' });
		},
		async ungroup(file) {
			await link.send({ kind: 'ungroup', titleId: fileIdentityKey(file) });
		},
		reorderSources(file, from, to) {
			link.post({ kind: 'reorderSources', titleId: fileIdentityKey(file), from, to });
		},
		view,
		capability,
		chooseCue(inputId, choice) {
			link.post({ kind: 'chooseCue', inputId, choice });
		},
		async importIntent(intent) {
			if (localError) {
				localError = '';
				changed();
			}
			try {
				await deps.beforeImport?.();
			} catch {
				localError = 'Could not load audio defaults. Review Settings, then try importing again.';
				changed();
				return;
			}
			await runImport(intent);
		},
		async hydrateSupportText() {
			try {
				const supported = await capabilityValue.getSupportedAudioImportMetadata();
				supportText = supported.supportText || supportText;
				changed();
			} catch {}
		},
		selectFile(command) {
			return applied({ kind: 'selectFile', index: command.index, modifiers: command.modifiers });
		},
		async selectAll() {
			await link.send({ kind: 'selectAll' });
		},
		async clearSelection() {
			await link.send({ kind: 'clearSelection' });
		},
		setDragOver(next) {
			if (isDragOver === next) return;
			isDragOver = next;
			changed();
		},
		async removeFile(index) {
			await link.send({ kind: 'removeFile', index });
		},
		async clearAllFiles() {
			await link.send({ kind: 'clearAll' });
		},
		moveFile(command) {
			link.post({ kind: 'moveFile', index: command.index, direction: command.direction });
		},
		reorderFiles(command) {
			link.post({ kind: 'reorderFiles', from: command.fromIndex, to: command.toIndex });
		},
		toggleSort() {
			link.post({ kind: 'toggleSort' });
		},
		restoreImportOrder() {
			link.post({ kind: 'restoreImportOrder' });
		},
		setOrderLocked(locked) {
			link.post({ kind: 'setOrderLocked', locked });
		},
		reset() {
			chosenAudio.clear();
			lastAudio.clear();
			localError = '';
			isDragOver = false;
			supportText = DEFAULT_SUPPORT_TEXT;
			changed();
			// The engine's session outlives this view; a new frontend attaches to it.
		},
	};
}

// Takes one output title's ordered sources; a title with several sources cannot carry CUE chapters.
export function chapterPlansForProcessing(
	sources: readonly AudioFile[],
): ProcessPayload['chapterPlans'] {
	const plans: NonNullable<ProcessPayload['chapterPlans']> = {};
	for (const file of sources.filter((file) => file.isValid)) {
		if (file.cueSource?.status === 'needsConfirmation' || file.cueSource?.status === 'invalid') {
			throw new Error(
				`Review ${file.cueSource.fileName}: confirm its timestamp interpretation or ignore the CUE before converting.`,
			);
		}
		if (sources.length > 1 && file.chapterPlan?.fromCue) {
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
