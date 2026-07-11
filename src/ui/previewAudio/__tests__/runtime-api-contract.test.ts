import { describe, expect, it } from 'vitest';
import * as previewAudio from '..';

describe('Preview Audio Runtime public API contract', () => {
	it('pins the preview controls public export strip', () => {
		expect(Object.keys(previewAudio).sort()).toEqual(['PreviewAudioControls']);
	});
});
