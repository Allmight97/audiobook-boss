import { intentLabel } from '../../app/encoding';
import type { AudioIntent, TitlePlan } from '../../lib/generated/tauri';

export type AudioPlanCopy = {
	readonly needsChoice: boolean;
	readonly kept: boolean;
	readonly glyph: string;
	readonly summary: string;
	readonly heading: string;
	readonly body: string;
};

export function audioPlanCopy(input: {
	readonly resolved: TitlePlan;
	readonly intent: AudioIntent;
	readonly codec: string;
	readonly format: string;
}): AudioPlanCopy {
	const { resolved, intent, codec, format } = input;
	const target = `${intent === 'encode' ? codec : intentLabel(intent)} · ${format}`;
	switch (resolved.kind) {
		case 'choiceRequired':
			return {
				needsChoice: true,
				kept: false,
				glyph: 'i',
				summary: 'Choose audio',
				heading: 'Choose this title’s audio',
				body: 'These sources had different audio settings. Choose settings for this title or apply App Settings.',
			};
		case 'failed':
			return {
				needsChoice: true,
				kept: false,
				glyph: 'i',
				summary: 'Choose audio',
				heading: 'Choose audio settings',
				body: resolved.message,
			};
		case 'pending':
			return {
				needsChoice: false,
				kept: false,
				glyph: 'i',
				summary: target,
				heading: 'Audio plan',
				body: 'Checking source audio…',
			};
		case 'resolved':
			return resolved.plan.handling === 'preserve'
				? {
						needsChoice: false,
						kept: true,
						glyph: '✓',
						summary: `Pass-through · ${format}`,
						heading: 'Keeping original audio',
						body: 'Audio stays unchanged. Tags, artwork and chapters will be updated.',
					}
				: {
						needsChoice: false,
						kept: false,
						glyph: 'i',
						summary: target,
						heading: `Using ${codec} with ${format}`,
						body: `Convert ${resolved.plan.sourceCodec} to ${codec} and save as ${format}.`,
					};
	}
}

export function channelLabel(channels: number): string {
	if (channels === 1) return 'Mono';
	if (channels === 2) return 'Stereo';
	return `${channels} channels`;
}
