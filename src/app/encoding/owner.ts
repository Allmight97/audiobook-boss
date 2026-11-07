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
import { encodingSummary } from './summary';

export type { EncodingField } from './edit';
export type { EncodingView } from './project';

export type PanelView = EncodingView & { readonly mixedFields: readonly EncodingField[] };

export type EncodingScope =
	| { readonly kind: 'defaults' }
	| { readonly kind: 'title'; readonly file: AudioFile }
	| { readonly kind: 'selection'; readonly files: readonly AudioFile[] };

/** One encoder panel's view, refusal, edit and summary line. */
export type EncodingForm = {
	/** `null` for a selection the engine has not combined yet. */
	readonly view: Accessor<PanelView | null>;
	/** Why the latest edit of these titles' audio changed nothing, worded. */
	readonly refusal: Accessor<string | null>;
	readonly choose: (field: EncodingField, value: string) => void;
	readonly summary: Accessor<string>;
};

export type EncodingOwner = {
	audioRequest: (file: AudioFile) => TitleAudioRequest;
	plan: (file: AudioFile) => TitlePlan;
	titleView: (file: AudioFile) => EncodingView;
	form: (scope: EncodingScope) => EncodingForm;
	applyDefaultsToTitles: (files: readonly AudioFile[]) => void;
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

	function selectionView(): PanelView | null {
		const selection = link.audio().selection;
		if (!selection) return null;
		return {
			...display(selection, {
				sampleRate: selection.autoSampleRate,
				channels: selection.autoChannels,
			}),
			mixedFields: selection.mixed,
		};
	}

	function refusal(files: readonly AudioFile[]): string | null {
		const refused = link.audio().refusal;
		const shown = new Set(files.map(titleId));
		if (!refused) return null;
		if (refused.kind === 'locked') {
			return "Audio can't change while an export is being prepared.";
		}
		const names = refused.labels.filter((_, index) => shown.has(refused.titleIds[index] ?? ''));
		if (names.length === 0) return null;
		return `${names.join(', ')} can't take that change, so no title changed.`;
	}

	function viewFor(scope: EncodingScope): PanelView | null {
		switch (scope.kind) {
			case 'defaults':
				return { ...display(link.audio().defaults, null), mixedFields: [] };
			case 'title':
				return { ...titleView(scope.file), mixedFields: [] };
			case 'selection':
				return selectionView();
		}
	}

	function form(scope: EncodingScope): EncodingForm {
		const view = () => viewFor(scope);
		const files = (): readonly AudioFile[] => {
			switch (scope.kind) {
				case 'defaults':
					return [];
				case 'title':
					return [scope.file];
				case 'selection':
					return scope.files;
			}
		};
		return {
			view,
			refusal: () => (scope.kind === 'defaults' ? null : refusal(files())),
			choose(field, value) {
				switch (scope.kind) {
					case 'defaults': {
						const edit = editFor(field, value, link.audio().defaults.facts);
						if (edit) link.post({ kind: 'setDefaultAudio', edit });
						return;
					}
					case 'title':
						sendTitleEdit([scope.file], field, value, titleChoice(scope.file).facts);
						return;
					case 'selection': {
						const [first] = scope.files;
						if (!first) return;
						const facts = link.audio().selection?.facts ?? titleChoice(first).facts;
						sendTitleEdit(scope.files, field, value, facts);
					}
				}
			},
			summary() {
				const shown = view();
				return shown ? encodingSummary(shown, shown.mixedFields) : '';
			},
		};
	}

	return {
		audioRequest(file) {
			return titleChoice(file).request;
		},
		plan,
		titleView,
		form,
		applyDefaultsToTitles(files) {
			if (files.length === 0) return;
			link.post({ kind: 'applyDefaultAudio', titleIds: files.map(titleId) });
		},
	};
}
