import { createSignal, type Accessor } from 'solid-js';
import type { AudioFile } from '../../types/audio';
import { toUserMessage } from '../../lib/tauri/appError';
import { liveInputCapability, type InputCapability } from '../../lib/tauri/capabilities/input';
import type { SessionIntent } from '../../types/session';
import type { EngineLink } from '../engineLink';
import { companionSummary, type CompanionSummary } from './companions';
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
	readonly view: Accessor<InputView>;
	readonly capability: Accessor<InputCapability>;
	sourcesFor(file: AudioFile): ReadonlyArray<AudioFile>;
	groupSelected(): Promise<void>;
	ungroup(file: AudioFile): Promise<void>;
	reorderSources(file: AudioFile, from: number, to: number): void;
	audioChoiceRequired(file: AudioFile): boolean;
	/** Whether a downloaded title has companion PDFs. */
	hasCompanions(inputId: string | undefined): boolean;
	companionSummary(inputIds: ReadonlyArray<string | undefined>): CompanionSummary;
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
	chooseCue(inputId: string, choice: 'confirmHundredths' | 'ignore'): void;
	reset(): void;
};

export type InputOwnerDeps = {
	readonly link: EngineLink;
	readonly capability?: InputCapability;
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

	async function applied(intent: SessionIntent): Promise<boolean> {
		return (await link.send(intent)).kind === 'applied';
	}

	async function importPaths(paths: ReadonlyArray<string>): Promise<void> {
		await link.send({ kind: 'import', paths: [...paths] });
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
				await link.send({ kind: 'importOpened' });
				return;
			case 'importPaths':
				await importPaths(intent.paths);
		}
	}

	return {
		sourcesFor,
		audioChoiceRequired(file) {
			return link.titles().audioChoiceRequired.includes(fileIdentityKey(file));
		},
		hasCompanions(inputId) {
			return Boolean(inputId && link.titles().companions[inputId]?.length);
		},
		companionSummary(inputIds) {
			return companionSummary(link.titles().companions, inputIds);
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
		reset() {
			localError = '';
			isDragOver = false;
			supportText = DEFAULT_SUPPORT_TEXT;
			changed();
			// The engine's session outlives this view; a new frontend attaches to it.
		},
	};
}
