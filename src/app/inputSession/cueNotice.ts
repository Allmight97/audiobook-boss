import type { AudioFile, CueChoice } from '../../lib/generated/tauri';
import { acceptedChapterCount } from './display';

export type CueNotice = {
	readonly summary: string | null;
	readonly message: string;
	readonly note: string | null;
	readonly actions: ReadonlyArray<{ readonly choice: CueChoice; readonly label: string }>;
};

const ACTION_LABELS: Record<CueChoice, string> = {
	confirmHundredths: 'Use hundredths',
	ignore: 'Ignore CUE',
};

export function cueNotice(file: AudioFile): CueNotice | null {
	const cue = file.cueSource;
	if (!cue) return null;
	const chapters = acceptedChapterCount(file);
	const source = cue.status === 'embeddedPreferred' ? 'embedded audio' : cue.fileName;
	const shown = cue.status !== 'ignored' && cue.status !== 'invalid';
	return {
		summary: shown ? `${chapters} chapter${chapters === 1 ? '' : 's'} from ${source}` : null,
		message: cue.message,
		note: cueNote(cue.status),
		actions: cue.actions.map((choice) => ({ choice, label: ACTION_LABELS[choice] })),
	};
}

function cueNote(status: NonNullable<AudioFile['cueSource']>['status']): string | null {
	switch (status) {
		case 'needsConfirmation':
			return 'This CUE uses nonstandard timestamps. Interpret the final field as hundredths of a second?';
		case 'ignored':
			return 'CUE ignored. Converting without its chapters.';
		case 'ready':
			return 'CUE chapters accepted.';
		case 'invalid':
		case 'embeddedPreferred':
			return null;
	}
}
