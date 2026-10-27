import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

// Enforces the removal date of each early-release exemption in bunfig.toml
// (scripts/AGENTS.md, Dependencies).
const bunfig = readFileSync(fileURLToPath(new URL('../bunfig.toml', import.meta.url)), 'utf8');

type Exemption = { readonly until: string; readonly packages: readonly string[] };

function exemptions(text: string): Exemption[] {
	const found: Exemption[] = [];
	let open: { until: string; packages: string[] } | null = null;
	for (const line of text.split('\n').map((each) => each.trim())) {
		const start = /^# early-install until (\d{4}-\d{2}-\d{2})$/.exec(line);
		if (start) {
			if (open) throw new Error(`early-install until ${open.until} has no end`);
			open = { until: start[1], packages: [] };
		} else if (line === '# early-install end') {
			if (!open) throw new Error('early-install end has no start');
			found.push(open);
			open = null;
		} else if (open && /^"[^"]+",?$/.test(line)) {
			open.packages.push(line.replace(/[",]/g, ''));
		}
	}
	if (open) throw new Error(`early-install until ${open.until} has no end`);
	return found;
}

function expired(list: readonly Exemption[], today: string): Exemption[] {
	return list.filter((exemption) => exemption.until < today);
}

describe('early-release exemptions', () => {
	it('each names its packages and is removed by its date', () => {
		const today = new Date().toISOString().slice(0, 10);
		for (const exemption of exemptions(bunfig)) expect(exemption.packages).not.toEqual([]);
		expect(expired(exemptions(bunfig), today)).toEqual([]);
	});

	it('catches a block past its date and a block without an end', () => {
		const block = '# early-install until 2026-11-03\n"vite",\n# early-install end';
		expect(expired(exemptions(block), '2026-11-03')).toEqual([]);
		expect(expired(exemptions(block), '2026-11-04')).toEqual([
			{ until: '2026-11-03', packages: ['vite'] },
		]);
		expect(() => exemptions('# early-install until 2026-11-03\n"vite",')).toThrow('no end');
	});
});
