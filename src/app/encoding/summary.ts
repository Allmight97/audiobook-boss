import type { EncodingField } from './edit';
import { codecFamilyLabel, encoderLabel } from './labels';
import type { EncodingView } from './project';

export function encodingSummary(view: EncodingView, mixedFields: readonly EncodingField[]): string {
	if (mixedFields.includes('encoder')) return 'Mixed encoders';
	if (mixedFields.includes('quality') || mixedFields.includes('bitrate')) return 'Mixed quality';
	if (view.codecFamily === 'opus') {
		return `${codecFamilyLabel(view.codecFamily)} · ${view.bitrate} kbps VBR`;
	}
	return `${encoderLabel(view.effectiveFlavor)} · ${view.showQuality ? `VBR ${view.quality}` : `${view.bitrate} kbps`}`;
}
