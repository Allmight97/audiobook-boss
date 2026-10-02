import { describe, expect, it } from 'vitest';
import { releaseProtocolLabel, uniqueDiagnosticMessage } from './display';

describe('remote source acquisition display', () => {
	it('does not leak duplicate diagnostic text into the status line', () => {
		const message = uniqueDiagnosticMessage([
			{ kind: 'downloadFailed', titleId: undefined, message: ' Token expired. ' },
			{ kind: 'downloadFailed', titleId: undefined, message: 'Token expired.' },
			{ kind: 'validationFailed', titleId: undefined, message: 'Retrying.' },
		]);
		expect(message).toBe('Token expired. Retrying.');
	});

	it('uses Prowlarr protocol words on release tags', () => {
		expect(releaseProtocolLabel('torrent')).toBe('torrent');
		expect(releaseProtocolLabel('usenet')).toBe('nzb');
		expect(releaseProtocolLabel('unknown')).toBe('unknown');
	});
});
