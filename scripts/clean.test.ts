import {
	chmodSync,
	closeSync,
	existsSync,
	mkdirSync,
	mkdtempSync,
	openSync,
	realpathSync,
	rmSync,
	symlinkSync,
	writeFileSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterAll, expect, it } from 'vitest';

import { autoClean, clean, KEPT_RUNS, linuxOpenPaths, systemOpenPaths } from './clean';

const roots: string[] = [];

function checkout(files: string[]): string {
	const root = mkdtempSync(path.join(os.tmpdir(), 'abb-clean-'));
	roots.push(root);
	for (const file of files) {
		mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
		writeFileSync(path.join(root, file), 'content');
	}
	return root;
}

const has = (root: string, file: string) => existsSync(path.join(root, file));

afterAll(() => {
	for (const root of roots) rmSync(root, { recursive: true, force: true });
});

const WHOLE_CHECKOUT = [
	'target/debug/deps/app',
	'target/release/bundle/app.dmg',
	'target/abb-ffmpeg-cache/tree/dist/lib.a',
	'target/abb-ffmpeg-source/commit.git/HEAD',
	'target/abb-ffmpeg-upstream/FFmpeg.git/HEAD',
	'dist/index.html',
	'node_modules/.vite/cache',
	'node_modules/solid-js/index.js',
	'.logs/final-verify.log',
	'.logs/runs/one/tauri-dev.log',
	'.logs/verification/run-1/engine.log',
	'.logs/verify/20261108T000000Z/export.log',
	'.logs/verification/receipts/engine.json',
	'.logs/untangle/decision-log.md',
	'src/main.ts',
];

const ALWAYS_KEPT = [
	'.logs/verification/receipts/engine.json',
	'.logs/untangle/decision-log.md',
	'src/main.ts',
];

it('removes build output and machine logs but keeps the FFmpeg cache, tools, and notes', () => {
	const root = checkout(WHOLE_CHECKOUT);
	symlinkSync('runs/one/tauri-dev.log', path.join(root, '.logs/tauri-dev.log'));

	clean(root, { all: false });

	for (const removed of [
		'target/debug',
		'target/release',
		'dist',
		'node_modules/.vite',
		'.logs/final-verify.log',
		'.logs/runs',
		'.logs/verification/run-1',
		'.logs/verify/20261108T000000Z',
	]) {
		expect(has(root, removed), removed).toBe(false);
	}
	for (const kept of [
		...ALWAYS_KEPT,
		'target/abb-ffmpeg-cache/tree/dist/lib.a',
		'target/abb-ffmpeg-source/commit.git/HEAD',
		'target/abb-ffmpeg-upstream/FFmpeg.git/HEAD',
		'node_modules/solid-js/index.js',
	]) {
		expect(has(root, kept), kept).toBe(true);
	}
});

it('--all also removes what setup restores, and still keeps notes and sources', () => {
	const root = checkout(WHOLE_CHECKOUT);

	clean(root, { all: true });

	for (const removed of ['target', 'node_modules']) {
		expect(has(root, removed), removed).toBe(false);
	}
	for (const kept of ALWAYS_KEPT) expect(has(root, kept), kept).toBe(true);
});

it('refuses to clean while a process holds a file open under target/', () => {
	const root = checkout(['target/debug/.cargo-lock', 'target/debug/deps/app']);
	const held = openSync(path.join(root, 'target/debug/.cargo-lock'), 'r');
	try {
		expect(() => clean(root, { all: false })).toThrow('A running process uses');
		expect(has(root, 'target/debug/deps/app')).toBe(true);
	} finally {
		closeSync(held);
	}
});

it('lists a file this process holds open', () => {
	const root = checkout(['target/debug/.cargo-lock']);
	const lock = path.join(realpathSync(root), 'target/debug/.cargo-lock');
	const held = openSync(lock, 'r');
	try {
		expect(systemOpenPaths()).toContain(lock);
	} finally {
		closeSync(held);
	}
});

it('reads a Linux process table in which some processes cannot be read', () => {
	const proc = mkdtempSync(path.join(os.tmpdir(), 'abb-proc-'));
	roots.push(proc);
	mkdirSync(path.join(proc, '100/fd'), { recursive: true });
	symlinkSync('/work/app', path.join(proc, '100/exe'));
	symlinkSync('/work/target/debug', path.join(proc, '100/cwd'));
	symlinkSync('/work/target/debug/.cargo-lock', path.join(proc, '100/fd/3'));
	mkdirSync(path.join(proc, '200/fd'), { recursive: true });
	chmodSync(path.join(proc, '200/fd'), 0o000);
	mkdirSync(path.join(proc, '300'));
	mkdirSync(path.join(proc, 'self-test'));

	try {
		expect(linuxOpenPaths(proc).toSorted()).toEqual([
			'/work/app',
			'/work/target/debug',
			'/work/target/debug/.cargo-lock',
		]);
	} finally {
		chmodSync(path.join(proc, '200/fd'), 0o755);
	}
});

it('cannot list processes when the process table is missing', () => {
	expect(() => linuxOpenPaths(path.join(os.tmpdir(), 'abb-no-such-proc'))).toThrow('not readable');
});

const runningFrom = (root: string, file: string) => () => [path.join(realpathSync(root), file)];

it('keeps target/debug under budget, and removes only it once over budget', () => {
	const root = checkout([
		'target/debug/deps/app',
		'target/abb-ffmpeg-cache/tree/dist/lib.a',
		'.logs/untangle/decision-log.md',
	]);

	expect(autoClean(root, Number.MAX_SAFE_INTEGER)).toEqual([]);
	expect(has(root, 'target/debug/deps/app')).toBe(true);

	expect(autoClean(root, 0).join()).toContain('Removed target/debug');
	expect(has(root, 'target/debug')).toBe(false);
	expect(has(root, 'target/abb-ffmpeg-cache/tree/dist/lib.a')).toBe(true);
	expect(has(root, '.logs/untangle/decision-log.md')).toBe(true);
});

it('says so and keeps target/debug when over budget while a process uses it', () => {
	const root = checkout(['target/debug/.cargo-lock', 'target/debug/deps/app']);
	const held = openSync(path.join(root, 'target/debug/.cargo-lock'), 'r');
	try {
		expect(autoClean(root, 0).join()).toContain('a running process uses target/debug/.cargo-lock');
		expect(has(root, 'target/debug/deps/app')).toBe(true);
	} finally {
		closeSync(held);
	}
});

it('keeps target/debug while a test binary runs from it, though no Cargo lock is held', () => {
	const root = checkout(['target/debug/deps/app']);

	const notes = autoClean(root, 0, runningFrom(root, 'target/debug/deps/app'));

	expect(notes.join()).toContain('a running process uses target/debug/deps/app');
	expect(has(root, 'target/debug/deps/app')).toBe(true);
});

it('keeps target/debug while a process has it as its working directory', () => {
	const root = checkout(['target/debug/deps/app']);
	const insideDebug = () => [realpathSync(path.join(root, 'target/debug/deps'))];

	autoClean(root, 0, insideDebug);

	expect(has(root, 'target/debug/deps/app')).toBe(true);
});

it('removes target/debug when a process uses only a sibling whose name starts the same', () => {
	const root = checkout(['target/debug/deps/app', 'target/debug-docs/index.html']);

	autoClean(root, 0, runningFrom(root, 'target/debug-docs/index.html'));

	expect(has(root, 'target/debug')).toBe(false);
});

it('says why and keeps target/debug when it cannot check for running processes', () => {
	const root = checkout(['target/debug/deps/app']);
	const unavailable = () => {
		throw new Error('lsof could not list running processes: spawn lsof ENOENT');
	};

	const notes = autoClean(root, 0, unavailable);

	expect(notes.join()).toContain('could not check for running processes');
	expect(notes.join()).toContain('spawn lsof ENOENT');
	expect(has(root, 'target/debug/deps/app')).toBe(true);
});

it('refuses to clean when it cannot check for running processes', () => {
	const root = checkout(['target/debug/deps/app']);
	const unavailable = () => {
		throw new Error('lsof could not list running processes: spawn lsof ENOENT');
	};

	expect(() => clean(root, { all: false }, unavailable)).toThrow('spawn lsof ENOENT');
	expect(has(root, 'target/debug/deps/app')).toBe(true);
});

it('keeps only the newest runs of each kind, and the receipts', () => {
	const extra = 3;
	const count = KEPT_RUNS + extra;
	const stamps = Array.from({ length: count }, (_, index) => `2026110${index}T000000Z`);
	const runs = [
		...stamps.map((stamp) => `.logs/verification/run-${stamp}-a`),
		...stamps.map((stamp) => `.logs/verify/${stamp}`),
	];
	const root = checkout([
		...runs.map((run) => `${run}/result.log`),
		'.logs/verification/receipts/engine.json',
	]);

	expect(autoClean(root)).toEqual([`Removed ${extra * 2} old verification runs.`]);

	for (const stamp of stamps.slice(0, extra)) {
		expect(has(root, `.logs/verification/run-${stamp}-a`), stamp).toBe(false);
		expect(has(root, `.logs/verify/${stamp}`), stamp).toBe(false);
	}
	for (const stamp of stamps.slice(extra)) {
		expect(has(root, `.logs/verification/run-${stamp}-a`), stamp).toBe(true);
		expect(has(root, `.logs/verify/${stamp}`), stamp).toBe(true);
	}
	expect(has(root, '.logs/verification/receipts/engine.json')).toBe(true);
});
