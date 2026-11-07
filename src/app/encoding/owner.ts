import type { Accessor } from 'solid-js';
import type {
	AudioChoiceFacts,
	AudioChoiceView,
	AudioFile,
	AutoReading,
	TitleAudioRequest,
	TitlePlan,
} from '../../lib/generated/tauri';
import type { EngineLink } from '../engineLink';
import { editFor, type EncodingField } from './edit';
import { projectView, type EncodingAuto, type EncodingView } from './project';

export type { EncodingField } from './edit';
export type { EncodingView } from './project';

export type EncodingOwner = {
	/** The request processing receives for `file`. */
	audioRequest: (file: AudioFile) => TitleAudioRequest;
	plan: (file: AudioFile) => TitlePlan;
	titleView: (file: AudioFile) => EncodingView;
	/** The selected titles' audio as the engine combines it; `null` with
	 * nothing selected. */
	selectionView: () => (EncodingView & { mixedFields: readonly EncodingField[] }) | null;
	/** Why the latest edit of `files`' audio changed nothing, worded. */
	refusal: (files: readonly AudioFile[]) => string | null;
	selectTitles: (files: readonly AudioFile[], field: EncodingField, value: string) => void;
	applyDefaultsToTitles: (files: readonly AudioFile[]) => void;
	selectTitle: (file: AudioFile, field: EncodingField, value: string) => void;
	/** The defaults, as the Settings panel edits them. */
	readonly view: Accessor<EncodingView>;
	select: (field: EncodingField, value: string) => void;
};

export type EncodingOwnerDeps = {
	readonly link: EngineLink;
};

const UNRESOLVED: AutoReading = { outcome: { kind: 'unresolved' }, problem: null };

function titleId(file: AudioFile): string {
	return file.inputId;
}

export function createEncodingOwner(deps: EncodingOwnerDeps): EncodingOwner {
	const { link } = deps;

	function titleChoice(file: AudioFile): AudioChoiceView {
		const audio = link.audio();
		return audio.titles[titleId(file)] ?? audio.defaults;
	}

	function plan(file: AudioFile): TitlePlan {
		return link.audio().titles[titleId(file)]?.plan ?? { kind: 'pending' };
	}

	function display(
		view: Pick<AudioChoiceView, 'choice' | 'facts'>,
		auto: EncodingAuto | null,
	): EncodingView {
		return projectView({
			choice: view.choice,
			facts: view.facts,
			capabilities: link.audio().capabilities,
			auto,
		});
	}

	function titleView(file: AudioFile): EncodingView {
		const audio = link.audio().titles[titleId(file)];
		return display(titleChoice(file), {
			sampleRate: audio?.autoSampleRate ?? UNRESOLVED,
			channels: audio?.autoChannels ?? UNRESOLVED,
		});
	}

	function sendTitleEdit(
		files: readonly AudioFile[],
		field: EncodingField,
		value: string,
		facts: AudioChoiceFacts,
	): void {
		const edit = editFor(field, value, facts);
		if (!edit || files.length === 0) return;
		link.post({ kind: 'setTitleAudio', titleIds: files.map(titleId), edit });
	}

	return {
		audioRequest(file) {
			return titleChoice(file).request;
		},
		plan,
		titleView,
		selectTitle(file, field, value) {
			sendTitleEdit([file], field, value, titleChoice(file).facts);
		},
		selectTitles(files, field, value) {
			const [first] = files;
			if (!first) return;
			sendTitleEdit(files, field, value, link.audio().selection?.facts ?? titleChoice(first).facts);
		},
		selectionView() {
			const selection = link.audio().selection;
			if (!selection) return null;
			return {
				...display(selection, {
					sampleRate: selection.autoSampleRate,
					channels: selection.autoChannels,
				}),
				mixedFields: selection.mixed,
			};
		},
		refusal(files) {
			const refusal = link.audio().refusal;
			const shown = new Set(files.map(titleId));
			if (!refusal) return null;
			if (refusal.kind === 'locked') {
				return "Audio can't change while an export is being prepared.";
			}
			const names = refusal.labels.filter((_, index) => shown.has(refusal.titleIds[index] ?? ''));
			if (names.length === 0) return null;
			return `${names.join(', ')} can't take that change, so no title changed.`;
		},
		applyDefaultsToTitles(files) {
			if (files.length === 0) return;
			link.post({ kind: 'applyDefaultAudio', titleIds: files.map(titleId) });
		},
		view: () => display(link.audio().defaults, null),
		select(field, value) {
			const edit = editFor(field, value, link.audio().defaults.facts);
			if (edit) link.post({ kind: 'setDefaultAudio', edit });
		},
	};
}
