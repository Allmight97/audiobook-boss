import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

describe('mock runtime production gate', () => {
	it('keeps production entry and HTML pointed at src/main.tsx', () => {
		const main = readFileSync(path.join(repoRoot, 'src/main.tsx'), 'utf8');
		const html = readFileSync(path.join(repoRoot, 'index.html'), 'utf8');
		expect(main).not.toMatch(/src\/mock|ui:mock|ABB_UI_MOCK/);
		expect(html).toContain('/src/main.tsx');
		expect(html).not.toContain('/src/mock/main.tsx');
	});

	it('gates the mock entry behind ABB_UI_MOCK in Vite and the ui:mock script', () => {
		const vite = readFileSync(path.join(repoRoot, 'vite.config.ts'), 'utf8');
		const pkg = JSON.parse(readFileSync(path.join(repoRoot, 'package.json'), 'utf8')) as {
			scripts: Record<string, string>;
		};
		expect(vite).toContain("process.env.ABB_UI_MOCK === '1'");
		expect(vite).toContain('/src/mock/main.tsx');
		expect(pkg.scripts['ui:mock']).toContain('ABB_UI_MOCK=1');
	});

	it('does not keep the deleted design lab', () => {
		expect(existsSync(path.join(repoRoot, 'lab.html'))).toBe(false);
		expect(existsSync(path.join(repoRoot, 'src/lab'))).toBe(false);
	});
});
