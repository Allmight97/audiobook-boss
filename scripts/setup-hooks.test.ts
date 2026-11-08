import { execFileSync, spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, expect, it } from 'vitest';

const repoRoot = path.resolve(import.meta.dirname, '..');
const scratch: string[] = [];

afterEach(() => {
	for (const dir of scratch.splice(0)) rmSync(dir, { recursive: true, force: true });
});

function git(root: string, args: string[]): void {
	execFileSync('git', args, { cwd: root, encoding: 'utf8' });
}

function repo(): string {
	const root = mkdtempSync(path.join(os.tmpdir(), 'abb-hooks-'));
	scratch.push(root);
	git(root, ['init', '-q']);
	return root;
}

function hooksInstalled(root: string): boolean {
	const script = [
		`source <(sed '/^require_prerequisites$/,$d' scripts/setup.sh)`,
		`git_hooks_installed ${JSON.stringify(root)}`,
	].join('\n');
	const result = spawnSync('bash', ['-c', script], { cwd: repoRoot, encoding: 'utf8' });
	return result.status === 0;
}

it('accepts this repo’s hooks path and a Cursor wrapper that still runs them', () => {
	const root = repo();
	expect(hooksInstalled(root)).toBe(false);

	git(root, ['config', 'core.hooksPath', '.githooks']);
	expect(hooksInstalled(root)).toBe(true);

	git(root, ['config', 'core.hooksPath', path.join(root, '.githooks')]);
	expect(hooksInstalled(root)).toBe(true);

	const wrapper = path.join(root, 'cursor-hooks');
	mkdirSync(wrapper);
	writeFileSync(
		path.join(wrapper, '.cursor-original-hooks-path'),
		`${path.join(root, '.githooks')}\n`,
	);
	git(root, ['config', 'core.hooksPath', wrapper]);
	expect(hooksInstalled(root)).toBe(true);

	writeFileSync(path.join(wrapper, '.cursor-original-hooks-path'), '.githooks\n');
	expect(hooksInstalled(root)).toBe(true);

	writeFileSync(path.join(wrapper, '.cursor-original-hooks-path'), '/tmp/other-hooks\n');
	expect(hooksInstalled(root)).toBe(false);
});
