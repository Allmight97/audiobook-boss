import { execFileSync, spawnSync } from 'node:child_process';
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

const repoRoot = path.resolve(import.meta.dirname, '..');
const pinnedUv = /^uv_version=(\S+)$/m.exec(
	readFileSync(path.join(repoRoot, 'scripts/setup.sh'), 'utf8'),
)?.[1];
const bashDir = path.dirname(execFileSync('which', ['bash'], { encoding: 'utf8' }).trim());
const scratch: string[] = [];

afterEach(() => {
	for (const dir of scratch.splice(0)) rmSync(dir, { recursive: true, force: true });
});

function ensureUvWith(installedVersion: string | null) {
	const home = mkdtempSync(path.join(os.tmpdir(), 'abb-setup-uv-'));
	scratch.push(home);
	const bin = path.join(home, 'bin');
	mkdirSync(bin);
	if (installedVersion) {
		const uvx = path.join(bin, 'uvx');
		writeFileSync(uvx, `#!/bin/bash\necho "uvx ${installedVersion} (test)"\n`);
		chmodSync(uvx, 0o755);
	}
	const script = [
		`source <(sed '/^require_prerequisites$/,$d' scripts/setup.sh)`,
		'fetch_verified() { echo "DOWNLOAD $1"; exit 42; }',
		'ensure_uv',
	].join('\n');
	return spawnSync('bash', ['-c', script], {
		cwd: repoRoot,
		encoding: 'utf8',
		env: {
			PATH: `${bin}:${bashDir}:/usr/bin:/bin`,
			HOME: home,
			ABB_TOOLS_BIN: path.join(home, 'tools'),
		},
	});
}

describe('uv pin', () => {
	it('keeps a uvx that reports the pinned version', () => {
		const result = ensureUvWith(pinnedUv ?? '');
		expect(result.status, result.stderr).toBe(0);
		expect(result.stdout).not.toContain('DOWNLOAD');
	});

	it('installs the pinned uv over a uvx of another version', () => {
		const result = ensureUvWith('0.0.1');
		expect(result.status, result.stderr).toBe(42);
		expect(result.stdout).toContain(
			`DOWNLOAD https://github.com/astral-sh/uv/releases/download/${pinnedUv}`,
		);
	});

	it('installs the pinned uv when none is on PATH', () => {
		expect(ensureUvWith(null).status).toBe(42);
	});
});

function toolsBinWith(override: string | null) {
	const home = mkdtempSync(path.join(os.tmpdir(), 'abb-setup-bin-'));
	scratch.push(home);
	const script = [
		`source <(sed '/^require_prerequisites$/,$d' scripts/setup.sh)`,
		'printf "%s\\n" "${tools_bin}"',
	].join('\n');
	const result = spawnSync('bash', ['-c', script], {
		cwd: repoRoot,
		encoding: 'utf8',
		env: {
			PATH: `${bashDir}:/usr/bin:/bin`,
			HOME: home,
			...(override ? { ABB_TOOLS_BIN: override } : {}),
		},
	});
	return { home, result, lines: result.stdout.split('\n') };
}

describe('tools folder', () => {
	it('defaults to an ABB-owned folder, never the shared ~/.local/bin', () => {
		const { home, result, lines } = toolsBinWith(null);
		expect(result.status, result.stderr).toBe(0);
		expect(lines[0]).toBe(path.join(home, '.local', 'share', 'audiobook-boss', 'bin'));
		expect(lines[0]).not.toBe(path.join(home, '.local', 'bin'));
	});

	it('is overridden by ABB_TOOLS_BIN (the folder setup installs into)', () => {
		const override = path.join(os.tmpdir(), 'abb-tools-override');
		const { result, lines } = toolsBinWith(override);
		expect(result.status, result.stderr).toBe(0);
		expect(lines[0]).toBe(override);
	});
});

describe('tool pins live in setup.sh', () => {
	const workflow = readFileSync(path.join(repoRoot, '.github/workflows/ci.yml'), 'utf8');

	it('has ci.yml call setup.sh instead of installing pinned tools itself', () => {
		expect(workflow).not.toMatch(/astral-sh\/setup-uv|taiki-e\/install-action|cargo-(audit|deny)@/);
		expect(workflow).toContain('bash scripts/setup.sh supply-chain');
	});
});
