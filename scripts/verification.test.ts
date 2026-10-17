import { execFileSync, spawnSync } from 'node:child_process';
import {
	copyFileSync,
	readFileSync,
	mkdtempSync,
	rmSync,
	writeFileSync,
	utimesSync,
	mkdirSync,
	symlinkSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { describe, expect, it } from 'vitest';
import {
	requiredLanes,
	planJobs,
	receiptMatches,
	gateFailures,
	fingerprintFiles,
	laneMap,
	explainPath,
	formatExplanation,
} from './verification';

const lanes = ['frontend', 'core', 'engine', 'tooling'];
const map = {
	frontend: ['src/**'],
	shared: ['Cargo.toml'],
	core: ['crates/*/src/**'],
	engine: ['crates/**'],
	tooling: ['docs/**'],
	'no-lane': ['LICENSE'],
};

describe('plan --explain', () => {
	const yaml = readFileSync(path.resolve(import.meta.dirname, 'lane-paths.yml'), 'utf8');
	const laneNames = execFileSync('bash', ['scripts/verify.sh', '--list'], {
		cwd: path.resolve(import.meta.dirname, '..'),
		encoding: 'utf8',
	})
		.trim()
		.split('\n');

	it('names the lane and pattern for a frontend path, an engine path, and a no-lane file', () => {
		expect(
			formatExplanation('src/app/entry.tsx', explainPath(yaml, laneNames, 'src/app/entry.tsx')),
		).toBe('src/app/entry.tsx\n  frontend  src/**');
		expect(
			explainPath(yaml, laneNames, 'crates/abb-engine/src/lib.rs').map((choice) => choice.lane),
		).toEqual(['core', 'engine', 'media', 'host', 'decrypt']);
		expect(explainPath(yaml, laneNames, 'docs/DECISIONS.md')).toEqual([
			{ lane: 'tooling', pattern: 'docs/**' },
		]);
		expect(formatExplanation('CHANGELOG.md', explainPath(yaml, laneNames, 'CHANGELOG.md'))).toBe(
			'CHANGELOG.md\n  no-lane  CHANGELOG.md  release history; the release skill writes it',
		);
	});
});

describe('requiredLanes', () => {
	it('rejects an unmapped file instead of reporting that no proof is needed', () => {
		expect(() => requiredLanes(map, lanes, ['future/worker.rs'])).toThrow(/future\/worker.rs/);
	});

	it('names every lane a changed file needs when nothing passed', () => {
		expect(requiredLanes(map, lanes, ['crates/abb-engine/src/a.rs'])).toEqual(['core', 'engine']);
	});
});

describe('platform job planning', () => {
	it('does not provision an empty Linux Rust job for core-only work', () => {
		expect(planJobs(['core'])).toEqual({
			frontend: false,
			core: true,
			tooling: false,
			supplyChain: false,
			rust: [{ os: 'macos-latest', lanes: ['core'], native: false, frontend: false }],
		});
	});
	it('adds Apple proof only when a native engine path is exercised', () => {
		expect(planJobs(['engine', 'media']).rust).toEqual([
			{ os: 'ubuntu-24.04', lanes: ['engine', 'media'], native: true, frontend: false },
			{ os: 'macos-latest', lanes: ['engine', 'media', 'apple'], native: true, frontend: false },
		]);
	});
});

describe('proof receipts', () => {
	it('accepts only a pass for the same input fingerprint', () => {
		expect(
			receiptMatches({ status: 'pass', fingerprint: 'same', tools: { bun: '1.4.3' } }, 'same'),
		).toBe(true);
		expect(
			receiptMatches({ status: 'pass', fingerprint: 'same', tools: { cc: 'unavailable' } }, 'same'),
		).toBe(false);
		for (const status of ['fail', 'skip', 'invalidated']) {
			expect(receiptMatches({ status, fingerprint: 'same', tools: { bun: '1.4.3' } }, 'same')).toBe(
				false,
			);
		}
		expect(
			receiptMatches({ status: 'pass', fingerprint: 'old', tools: { bun: '1.4.3' } }, 'changed'),
		).toBe(false);
	});
});

describe('validation boundaries', () => {
	it('does not accept a shared anchor as an executable lane', () => {
		expect(() => requiredLanes(map, lanes, ['Cargo.toml'])).toThrow(/Cargo.toml/);
	});
	it('rejects unknown lane names and unexplained exemptions', () => {
		expect(() => laneMap('typo:\n - src/**\n', lanes)).toThrow(/Unknown lane/);
		expect(() => laneMap('no-lane:\n - LICENSE\n', lanes)).toThrow(/reason/);
	});
	it('rejects a selected job that was skipped even when every other job passed', () => {
		const jobs = planJobs(['frontend']);
		const results = Object.fromEntries(
			['changes', 'frontend', 'core', 'rust', 'tooling', 'supply-chain'].map((job) => [
				job,
				{ result: job === 'changes' ? 'success' : 'skipped' },
			]),
		);
		expect(gateFailures(jobs, results)).toEqual(['frontend: expected success, received skipped']);
		results.frontend.result = 'success';
		expect(gateFailures(jobs, results)).toEqual([]);
	});
	it('invalidates proof for content changes with unchanged timestamps, deletions, and environment changes', () => {
		const root = mkdtempSync(path.join(os.tmpdir(), 'abb-proof-'));
		try {
			const file = path.join(root, 'input.rs');
			writeFileSync(file, 'before');
			utimesSync(file, 10, 10);
			const before = fingerprintFiles(root, ['input.rs'], { toolchain: 'one' });
			writeFileSync(file, 'after');
			utimesSync(file, 10, 10);
			expect(fingerprintFiles(root, ['input.rs'], { toolchain: 'one' })).not.toBe(before);
			expect(fingerprintFiles(root, ['input.rs'], { toolchain: 'two' })).not.toBe(
				fingerprintFiles(root, ['input.rs'], { toolchain: 'one' }),
			);
			rmSync(file);
			expect(fingerprintFiles(root, ['input.rs'], { toolchain: 'one' })).not.toBe(before);
		} finally {
			rmSync(root, { recursive: true, force: true });
		}
	});
});

it('fingerprints directory aliases such as .claude/skills without reading a directory as a file', () => {
	const root = mkdtempSync(path.join(os.tmpdir(), 'abb-link-proof-'));
	try {
		mkdirSync(path.join(root, 'skills'));
		symlinkSync('skills', path.join(root, 'alias'));
		expect(fingerprintFiles(root, ['alias'], {})).toMatch(/^[a-f0-9]{64}$/);
	} finally {
		rmSync(root, { recursive: true, force: true });
	}
});

describe('verification execution and harness handoff', () => {
	it('retains failure evidence, lets the done-gate from any harness accept the proof, and invalidates an edited input', () => {
		const root = mkdtempSync(path.join(os.tmpdir(), 'abb-verify-run-'));
		const sourceRoot = path.resolve(import.meta.dirname, '..');
		try {
			for (const directory of ['scripts', 'src', 'bin']) mkdirSync(path.join(root, directory));
			for (const file of ['verify.sh', 'verification.ts'])
				copyFileSync(path.join(sourceRoot, 'scripts', file), path.join(root, 'scripts', file));
			writeFileSync(path.join(root, '.gitignore'), '.logs/\nnode_modules\nbin/\n');
			symlinkSync(path.join(sourceRoot, 'node_modules'), path.join(root, 'node_modules'));
			writeFileSync(
				path.join(root, 'scripts/setup.sh'),
				`#!/bin/bash
if [[ "$1" == --print-env ]]; then printf 'export ABB_FFMPEG=/setup/ffmpeg\\nexport PATH="/setup/bin:%s"\\n' "$PATH"; fi
`,
				{ mode: 0o755 },
			);
			mkdirSync(path.join(root, '.claude'));
			copyFileSync(
				path.join(sourceRoot, '.claude/done-gate.json'),
				path.join(root, '.claude/done-gate.json'),
			);
			writeFileSync(path.join(root, 'scripts/lane-paths.yml'), 'frontend:\n - "**"\n');
			writeFileSync(path.join(root, 'src/input.ts'), 'before');
			const realBun = execFileSync('which', ['bun'], { encoding: 'utf8' }).trim();
			writeFileSync(
				path.join(root, 'bin/bun'),
				`#!/bin/bash
if [[ "$1" == --version ]]; then printf 'installer progress: %s\\n' "$RANDOM" >&2; exec "$REAL_BUN" "$@"; fi
if [[ "$1" == scripts/verification.ts ]]; then exec "$REAL_BUN" "$@"; fi
printf 'executed: %s\\n' "$*"
if [[ "$*" == 'run lint:check' ]]; then exit "${'$'}{VERIFY_TEST_EXIT:-0}"; fi
`,
				{ mode: 0o755 },
			);
			execFileSync('git', ['init', '-q'], { cwd: root });
			execFileSync('git', ['add', '.'], { cwd: root });
			execFileSync(
				'git',
				[
					'-c',
					'user.name=Test',
					'-c',
					'user.email=test@example.invalid',
					'commit',
					'-qm',
					'fixture',
				],
				{ cwd: root },
			);
			const env = { ...process.env, REAL_BUN: realBun, PATH: `${root}/bin:${process.env.PATH}` };
			const run = (code: string) =>
				spawnSync('bash', ['scripts/verify.sh', 'frontend'], {
					cwd: root,
					env: { ...env, VERIFY_TEST_EXIT: code },
					encoding: 'utf8',
				});
			expect(run('9').status).toBe(1);
			const receiptFile = path.join(
				root,
				'.logs/verification/receipts',
				`${process.platform}-${process.arch}-frontend.json`,
			);
			const failure = JSON.parse(readFileSync(receiptFile, 'utf8'));
			expect(failure.status).toBe('fail');
			expect(readFileSync(path.join(root, failure.log), 'utf8')).toContain('run lint:check');
			expect(run('0').status).toBe(0);
			const [command, ...args] = JSON.parse(
				readFileSync(path.join(root, '.claude/done-gate.json'), 'utf8'),
			).resolve as string[];
			const resolve = (harnessPath = env.PATH) =>
				spawnSync(command, args, {
					cwd: root,
					env: { ...env, PATH: harnessPath },
					encoding: 'utf8',
					input: JSON.stringify({ changed: ['src/input.ts'], passes: [] }),
				});
			expect(resolve().stdout).toBe('');
			expect(resolve(`${env.PATH}:/another-harness/bin`).stdout).toBe('');
			expect(JSON.parse(readFileSync(receiptFile, 'utf8')).status).toBe('pass');
			writeFileSync(path.join(root, 'src/input.ts'), 'after');
			expect(resolve().stdout.trim()).toBe('bash scripts/verify.sh frontend');
		} finally {
			rmSync(root, { recursive: true, force: true });
		}
	});
});
