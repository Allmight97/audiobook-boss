import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import { laneMap, planJobs, requiredLanes } from './verification';

const script = path.resolve(import.meta.dirname, 'check-file-size.mjs');
const repoRoot = path.resolve(import.meta.dirname, '..');
const budgetBytes = 8000 * 4;
const over = budgetBytes + 1;

function git(root: string, args: string[]): string {
	return execFileSync('git', args, { cwd: root, encoding: 'utf8' });
}

function tempRepo(): string {
	const root = mkdtempSync(path.join(os.tmpdir(), 'abb-file-size-'));
	git(root, ['init', '-q', '-b', 'main']);
	git(root, ['config', 'user.email', 'ratchet@example.com']);
	git(root, ['config', 'user.name', 'Ratchet Test']);
	git(root, ['config', 'commit.gpgsign', 'false']);
	git(root, ['config', 'core.fsmonitor', 'false']);
	const hooks = path.join(root, 'no-hooks');
	mkdirSync(hooks);
	git(root, ['config', 'core.hooksPath', hooks]);
	return root;
}

function writeBytes(root: string, file: string, size: number, prefix = ''): void {
	const full = path.join(root, file);
	mkdirSync(path.dirname(full), { recursive: true });
	const head = Buffer.from(prefix);
	if (head.length > size) throw new Error(`prefix longer than ${size}`);
	writeFileSync(full, Buffer.concat([head, Buffer.alloc(size - head.length, 'a')]));
}

function commitAll(root: string, message: string): string {
	git(root, ['add', '-A']);
	git(root, ['commit', '-q', '-m', message]);
	return git(root, ['rev-parse', 'HEAD']).trim();
}

function run(root: string, args: string[]) {
	return spawnSync('bun', [script, ...args], { cwd: root, encoding: 'utf8' });
}

function withRepo(body: (root: string) => void): void {
	const root = tempRepo();
	try {
		body(root);
	} finally {
		rmSync(root, { recursive: true, force: true });
	}
}

describe('check-file-size --against', () => {
	it('fails a new source file over the budget', () => {
		withRepo((root) => {
			writeBytes(root, 'keep.rs', 20);
			const base = commitAll(root, 'base');
			writeBytes(root, 'big.rs', over);
			commitAll(root, 'add big');
			const result = run(root, ['--against', base]);
			expect(result.status, result.stderr).toBe(1);
			expect(result.stderr).toContain('big.rs');
			expect(result.stderr).toContain('a new file');
			expect(result.stderr).toContain('Split a file before adding to it');
		});
	});

	it('fails when an already over-budget file grows, and passes a shrink or no change', () => {
		withRepo((root) => {
			writeBytes(root, 'god.rs', over + 1000);
			const base = commitAll(root, 'over');
			writeBytes(root, 'note.txt', 10);
			commitAll(root, 'unrelated');
			expect(run(root, ['--against', base]).status).toBe(0);

			writeBytes(root, 'god.rs', over + 2000);
			commitAll(root, 'grow');
			const grew = run(root, ['--against', base]);
			expect(grew.status, grew.stderr).toBe(1);
			expect(grew.stderr).toContain('may not grow');

			writeBytes(root, 'god.rs', over + 10);
			commitAll(root, 'shrink');
			expect(run(root, ['--against', base]).status, 'shrink').toBe(0);
		});
	});

	it('fails a file that crosses the budget, and ignores growth that stays under it', () => {
		withRepo((root) => {
			writeBytes(root, 'small.rs', 100);
			const base = commitAll(root, 'small');
			writeBytes(root, 'small.rs', 200);
			commitAll(root, 'still small');
			expect(run(root, ['--against', base]).status).toBe(0);
			writeBytes(root, 'small.rs', over);
			commitAll(root, 'crossed');
			expect(run(root, ['--against', base]).status).toBe(1);
		});
	});

	it('does not fail a checkout whose index and HEAD match when the growth is already committed', () => {
		withRepo((root) => {
			writeBytes(root, 'god.rs', over);
			const base = commitAll(root, 'over');
			writeBytes(root, 'god.rs', over + 50);
			commitAll(root, 'grow');
			expect(run(root, ['god.rs']).status, 'staged mode sees no growth against HEAD').toBe(0);
			expect(run(root, ['--against', base]).status).toBe(1);
		});
	});

	it('passes when HEAD is the base, even if a source file is already over the budget', () => {
		withRepo((root) => {
			writeBytes(root, 'god.rs', over);
			commitAll(root, 'over');
			expect(run(root, ['--against', 'HEAD']).status).toBe(0);
		});
	});

	it('measures a rename against the path it came from', () => {
		withRepo((root) => {
			writeBytes(root, 'old.rs', over);
			const base = commitAll(root, 'over');
			git(root, ['mv', 'old.rs', 'new.rs']);
			writeBytes(root, 'new.rs', over + 80);
			commitAll(root, 'rename and grow');
			const result = run(root, ['--against', base]);
			expect(result.status, result.stderr).toBe(1);
			expect(result.stderr).toContain('new.rs');
		});
	});

	it('skips linguist-vendored and linguist-generated paths', () => {
		withRepo((root) => {
			writeFileSync(
				path.join(root, '.gitattributes'),
				'vendor/** linguist-vendored\nsrc/lib/generated/** linguist-generated\n',
			);
			writeBytes(root, 'vendor/frame.c', over);
			writeBytes(root, 'src/lib/generated/tauri.ts', over);
			const base = commitAll(root, 'skipped');
			expect(run(root, ['--all']).status, 'only skipped files').toBe(0);
			writeBytes(root, 'src/authored.rs', over);
			commitAll(root, 'authored');
			const listed = run(root, ['--all']);
			expect(listed.status).toBe(1);
			expect(listed.stdout).toContain('src/authored.rs');
			expect(listed.stdout).not.toContain('vendor/frame.c');
			expect(listed.stdout).not.toContain('src/lib/generated/tauri.ts');
			const gated = run(root, ['--against', base]);
			expect(gated.status, gated.stderr).toBe(1);
			expect(gated.stderr).toContain('src/authored.rs');
			expect(gated.stderr).not.toContain('vendor/frame.c');
			expect(gated.stderr).not.toContain('tauri.ts');
		});
	});

	it('skips a file that opts out, and still counts it in --all', () => {
		withRepo((root) => {
			writeBytes(root, 'keep.rs', 20);
			const base = commitAll(root, 'base');
			writeBytes(root, 'table.rs', over, '// @ratchet-files-ignore: one opcode table.\n');
			commitAll(root, 'opt out');
			const listed = run(root, ['--all']);
			expect(listed.status, listed.stderr).toBe(0);
			expect(listed.stdout).toContain('opted out');
			expect(run(root, ['--against', base]).status).toBe(0);
		});
	});

	it('fails closed when the base object is missing', () => {
		withRepo((root) => {
			writeBytes(root, 'small.rs', 20);
			commitAll(root, 'base');
			const missing = '0123456789abcdef0123456789abcdef01234567';
			const result = run(root, ['--against', missing]);
			expect(result.status, result.stderr).toBe(1);
			expect(result.stderr).toContain('fails closed');
			expect(result.stderr).not.toContain('Commit refused');
		});
	});

	it('fails closed when the base has no common ancestor', () => {
		withRepo((root) => {
			writeBytes(root, 'small.rs', 20);
			commitAll(root, 'ours');
			const other = tempRepo();
			try {
				writeBytes(other, 'other.rs', 20);
				commitAll(other, 'theirs');
				git(root, ['remote', 'add', 'other', other]);
				git(root, ['fetch', '-q', 'other', 'main']);
				const result = run(root, ['--against', 'FETCH_HEAD']);
				expect(result.status, result.stderr).toBe(1);
				expect(result.stderr).toContain('no merge base');
				expect(result.stderr).toContain('fails closed');
			} finally {
				rmSync(other, { recursive: true, force: true });
			}
		});
	});
});

describe('check-file-size staged', () => {
	it('refuses a staged new file over the budget and a staged growth of an over-budget file', () => {
		withRepo((root) => {
			writeBytes(root, 'small.rs', 20);
			commitAll(root, 'base');
			writeBytes(root, 'big.rs', over);
			git(root, ['add', 'big.rs']);
			const added = run(root, ['big.rs']);
			expect(added.status, added.stderr).toBe(1);
			git(root, ['commit', '-q', '-m', 'big']);
			expect(run(root, ['big.rs']).status).toBe(0);
			writeBytes(root, 'big.rs', over + 40);
			git(root, ['add', 'big.rs']);
			const grew = run(root, ['big.rs']);
			expect(grew.status, grew.stderr).toBe(1);
			expect(grew.stderr).toContain('may not grow');
		});
	});
});

describe('file-size ratchet selection', () => {
	it('runs the tooling job for a changed Rust source file and a changed TypeScript source file', () => {
		const yaml = readFileSync(path.join(repoRoot, 'scripts/lane-paths.yml'), 'utf8');
		const lanes = execFileSync('bash', ['scripts/verify.sh', '--list'], {
			cwd: repoRoot,
			encoding: 'utf8',
		})
			.trim()
			.split('\n');
		const map = laneMap(yaml, lanes);
		for (const file of ['crates/abb-engine/src/session/runtime.rs', 'src/lib/tauri/client.ts']) {
			const selected = requiredLanes(map, lanes, [file]);
			expect(selected, file).toContain('tooling');
			expect(planJobs(selected).tooling, file).toBe(true);
		}
	});
});
