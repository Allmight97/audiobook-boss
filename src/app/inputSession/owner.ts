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

export type InputOwner = {
	readonly view: Accessor<InputView>;
	readonly capability: Accessor<InputCapability>;
	sourcesFor(file: AudioFile): ReadonlyArray<AudioFile>;
	groupSelected(): void;
	ungroup(file: AudioFile): void;
	reorderSources(file: AudioFile, from: number, to: number): void;
	audioChoiceRequired(file: AudioFile): boolean;
	hasCompanions(inputId: string | undefined): boolean;
	companionSummary(inputIds: ReadonlyArray<string | undefined>): CompanionSummary;
	/** Resolves once the picker closes and the import is sent, before it lands. */
	importIntent(intent: ImportIntent): Promise<void>;
	hydrateSupportText(): Promise<void>;
	/** Resolves true when the selection changed and its tags have loaded. */
	selectFile(command: {
		readonly index: number;
		readonly modifiers: SelectionModifiers;
	}): Promise<boolean>;
	selectAll(): void;
	clearSelection(): void;
	setDragOver(isDragOver: boolean): void;
	removeFile(file: AudioFile): void;
	clearAllFiles(): void;
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
		try {
			return (await link.send(intent)).kind === 'applied';
		} catch (error) {
			link.refuse(intent, error);
			return false;
		}
	}

	function importPaths(paths: ReadonlyArray<string>): void {
		link.post({ kind: 'import', paths: [...paths] });
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
				if (selected?.length) importPaths(selected);
				return;
			}
			case 'pickFolder': {
				const selected = await pick(
					() => capabilityValue.openDirectory(),
					'Failed to open folder dialog. Please try again.',
				);
				if (selected) importPaths([selected]);
				return;
			}
			case 'importPaths':
				importPaths(intent.paths);
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
		groupSelected() {
			link.post({ kind: 'groupSelected' });
		},
		ungroup(file) {
			link.post({ kind: 'ungroup', titleId: fileIdentityKey(file) });
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
			const file = link.titles().files[command.index];
			if (!file) return Promise.resolve(false);
			return applied({
				kind: 'selectFile',
				titleId: fileIdentityKey(file),
				modifiers: command.modifiers,
			});
		},
		selectAll() {
			link.post({ kind: 'selectAll' });
		},
		clearSelection() {
			link.post({ kind: 'clearSelection' });
		},
		setDragOver(next) {
			if (isDragOver === next) return;
			isDragOver = next;
			changed();
		},
		removeFile(file) {
			link.post({ kind: 'removeFile', inputId: fileIdentityKey(file) });
		},
		clearAllFiles() {
			link.post({ kind: 'clearAll' });
		},
		moveFile(command) {
			const file = link.titles().files[command.index];
			if (!file) return;
			link.post({
				kind: 'moveFile',
				titleId: fileIdentityKey(file),
				direction: command.direction,
			});
		},
		reorderFiles(command) {
			const file = link.titles().files[command.fromIndex];
			if (!file) return;
			link.post({ kind: 'reorderFiles', titleId: fileIdentityKey(file), to: command.toIndex });
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
		},
	};
}
