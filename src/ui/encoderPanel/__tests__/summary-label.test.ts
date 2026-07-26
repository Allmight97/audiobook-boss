import { beforeEach, describe, expect, it } from 'vitest';
import { readEncoderSummaryLabel } from '..';
import { encoderPanelState, resetEncoderPanelState } from '../state.svelte';

describe('readEncoderSummaryLabel', () => {
	beforeEach(() => resetEncoderPanelState());

	it('derives the selected encoder, profile, and VBR quality from live panel state', () => {
		encoderPanelState.flavor = 'fdk_he_aac';
		encoderPanelState.profileDisplay = 'HE-AAC v1';
		encoderPanelState.bitrateModeSelection = 'vbr';
		encoderPanelState.qualityValue = 3;

		expect(readEncoderSummaryLabel()).toBe('FDK HE-AAC · VBR 3');
	});

	it('derives bitrate modes from the live bitrate selection', () => {
		encoderPanelState.flavor = 'aac_at';
		encoderPanelState.profileDisplay = 'AAC-LC';
		encoderPanelState.bitrateModeSelection = 'cbr';
		encoderPanelState.bitrateValue = 96;

		expect(readEncoderSummaryLabel()).toBe('Apple AAC-LC · CBR 96 kbps');
	});
});
