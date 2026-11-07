import { afterEach, describe, expect, it, vi } from 'vitest';
import { fileManagerLabel, multiSelectClickLabel, runsOnMac } from '.';

const MAC = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15';
const LINUX = 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15';

function runOn(userAgent: string): void {
	vi.spyOn(navigator, 'userAgent', 'get').mockReturnValue(userAgent);
}

afterEach(() => vi.restoreAllMocks());

describe('platform wording', () => {
	it('names Finder and the Command key on macOS', () => {
		runOn(MAC);
		expect(runsOnMac()).toBe(true);
		expect(fileManagerLabel()).toBe('Show in Finder');
		expect(multiSelectClickLabel()).toBe('⌘-click');
	});

	it('names the folder and the Ctrl key elsewhere', () => {
		runOn(LINUX);
		expect(runsOnMac()).toBe(false);
		expect(fileManagerLabel()).toBe('Show in Folder');
		expect(multiSelectClickLabel()).toBe('Ctrl-click');
	});
});
