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

describe('uv install', () => {
	it('keeps any uvx already on PATH, whatever its version', () => {
		for (const version of [pinnedUv ?? '', '0.0.1']) {
			const result = ensureUvWith(version);
			expect(result.status, result.stderr).toBe(0);
			expect(result.stdout).not.toContain('DOWNLOAD');
			expect(result.stdout).toContain(`Using uv ${version}`);
		}
	});

	it('installs the pinned uv when none is on PATH', () => {
		const result = ensureUvWith(null);
		expect(result.status, result.stderr).toBe(42);
		expect(result.stdout).toContain(
			`DOWNLOAD https://github.com/astral-sh/uv/releases/download/${pinnedUv}`,
		);
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

const setupSh = readFileSync(path.join(repoRoot, 'scripts/setup.sh'), 'utf8');
const pinnedFfmpegRelease = /^ffmpeg_cli_release=(\S+)$/m.exec(setupSh)?.[1];
const pinnedFfmpegBuild = /^ffmpeg_cli_build=(\S+)$/m.exec(setupSh)?.[1];

function fakeCli(bin: string, name: string, version: string) {
	mkdirSync(bin, { recursive: true });
	const file = path.join(bin, name);
	writeFileSync(file, `#!/bin/bash\necho "ffmpeg version ${version} Copyright"\n`);
	chmodSync(file, 0o755);
}

function readbackCli(version: string) {
	const home = mkdtempSync(path.join(os.tmpdir(), 'abb-setup-ffmpeg-'));
	scratch.push(home);
	const tools = path.join(home, 'tools');
	fakeCli(tools, 'ffmpeg', version);
	fakeCli(tools, 'ffprobe', version);
	const script = [
		`source <(sed '/^require_prerequisites$/,$d' scripts/setup.sh)`,
		'is_darwin() { return 1; }',
		'uname() { if [[ "$1" == -m ]]; then printf x86_64; else printf Linux; fi; }',
		'fetch_verified() { printf "DOWNLOAD %s %s\\n" "$1" "$2"; exit 42; }',
		'ensure_readback_cli',
	].join('\n');
	return spawnSync('bash', ['-c', script], {
		cwd: repoRoot,
		encoding: 'utf8',
		env: {
			PATH: `${bashDir}:/usr/bin:/bin`,
			HOME: home,
			ABB_TOOLS_BIN: tools,
		},
	});
}

describe('FFmpeg readback CLI', () => {
	it('names the BtbN autobuild of the bundled FFmpeg commit', () => {
		const revision = readFileSync(
			path.join(repoRoot, 'vendor/ffmpeg-sys-next-9.0.1/ffmpeg-revision'),
			'utf8',
		).trim();
		const short = pinnedFfmpegBuild?.match(/g([0-9a-f]+)$/)?.[1];
		expect(short && revision.startsWith(short)).toBe(true);
		expect(pinnedFfmpegRelease).toMatch(/^autobuild-/);
	});

	it('installs the pinned build when the tools folder has a different FFmpeg 9', () => {
		const result = readbackCli('n9.0.2-39-g7adfbe40dc-20261031');
		expect(result.status, result.stderr).toBe(42);
		expect(result.stdout).toContain(
			`DOWNLOAD https://github.com/FFmpeg-Builds/FFmpeg-Builds/releases/download/${pinnedFfmpegRelease} ffmpeg-${pinnedFfmpegBuild}-linux64-gpl.tar.xz`,
		);
	});

	it('keeps the tools-folder CLI when its version is the pinned build', () => {
		const result = readbackCli(`${pinnedFfmpegBuild}-20261109`);
		expect(result.status, result.stderr).toBe(0);
		expect(result.stdout).not.toContain('DOWNLOAD');
		expect(result.stdout).toContain(`Using FFmpeg readback CLI ${pinnedFfmpegBuild}`);
	});
});

describe('tool pins live in setup.sh', () => {
	const workflow = readFileSync(path.join(repoRoot, '.github/workflows/ci.yml'), 'utf8');

	it('has ci.yml call setup.sh instead of installing pinned tools itself', () => {
		expect(workflow).not.toMatch(/astral-sh\/setup-uv|taiki-e\/install-action|cargo-(audit|deny)@/);
		expect(workflow).toContain('bash scripts/setup.sh supply-chain');
	});
});
