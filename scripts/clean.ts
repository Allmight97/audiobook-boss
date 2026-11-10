import { spawnSync } from 'node:child_process';
import { existsSync, readdirSync, rmSync } from 'node:fs';
import path from 'node:path';

const USAGE = `Usage: bun run clean [--all]
       bun scripts/clean.ts --auto

  (no flag)  Remove build output and machine-written logs. Keeps the FFmpeg cache.
  --all      Also remove the FFmpeg cache and node_modules.
             bash scripts/setup.sh restores them.
  --auto     Remove target/debug when it is over budget and keep the newest
             verification runs. verify.sh and app:dev:log run this.

Other folders under .logs/ and verification receipts always stay.`;

const FFMPEG_DIRS = new Set(['abb-ffmpeg-cache', 'abb-ffmpeg-source']);
const BUILD_OUTPUT = ['dist', 'node_modules/.vite'];

const DEBUG_BUDGET_KB = 25 * 1024 * 1024;
export const KEPT_RUNS = 5;

function remove(root: string, relative: string): void {
	rmSync(path.join(root, relative), { recursive: true, force: true });
}

function entriesOf(directory: string) {
	return existsSync(directory) ? readdirSync(directory, { withFileTypes: true }) : [];
}

function cargoIsBuilding(root: string): boolean {
	return entriesOf(path.join(root, 'target')).some((entry) => {
		const lock = path.join(root, 'target', entry.name, '.cargo-lock');
		if (!existsSync(lock)) return false;
		const result = spawnSync('lsof', ['-t', lock], { encoding: 'utf8' });
		if (result.error) throw result.error;
		return result.stdout.trim() !== '';
	});
}

function runsOldestFirst(root: string): { verification: string[]; verifyAbb: string[] } {
	const folders = (relative: string) =>
		entriesOf(path.join(root, relative))
			.filter((entry) => entry.isDirectory())
			.map((entry) => path.join(relative, entry.name))
			.toSorted();
	return {
		verification: folders('.logs/verification').filter((run) =>
			path.basename(run).startsWith('run-'),
		),
		verifyAbb: folders('.logs/verify'),
	};
}

function removeLogs(root: string): void {
	for (const entry of entriesOf(path.join(root, '.logs'))) {
		if (!entry.isDirectory()) remove(root, path.join('.logs', entry.name));
	}
	remove(root, '.logs/runs');
	const { verification, verifyAbb } = runsOldestFirst(root);
	for (const run of [...verification, ...verifyAbb]) remove(root, run);
}

export function clean(root: string, { all }: { all: boolean }): void {
	if (cargoIsBuilding(root))
		throw new Error('Cargo is building in this checkout. Nothing removed.');
	for (const entry of entriesOf(path.join(root, 'target'))) {
		if (!FFMPEG_DIRS.has(entry.name)) remove(root, path.join('target', entry.name));
	}
	for (const relative of BUILD_OUTPUT) remove(root, relative);
	removeLogs(root);
	if (!all) return;
	remove(root, 'target');
	remove(root, 'node_modules');
}

function sizeKb(directory: string): number {
	const result = spawnSync('du', ['-sk', directory], { encoding: 'utf8' });
	if (result.error) throw result.error;
	const size = Number.parseInt(result.stdout, 10);
	if (result.status !== 0 || Number.isNaN(size)) throw new Error(`du could not size ${directory}`);
	return size;
}

export function autoClean(root: string, budgetKb: number = DEBUG_BUDGET_KB): string[] {
	const { verification, verifyAbb } = runsOldestFirst(root);
	const removed = [verification, verifyAbb].flatMap((runs) => runs.slice(0, -KEPT_RUNS));
	for (const run of removed) remove(root, run);
	const notes = removed.length > 0 ? [`Removed ${removed.length} old verification runs.`] : [];

	const debug = path.join(root, 'target/debug');
	if (!existsSync(debug)) return notes;
	const size = sizeKb(debug);
	if (size <= budgetKb) return notes;
	const gigabytes = Math.round(size / (1024 * 1024));
	if (cargoIsBuilding(root)) {
		return [...notes, `target/debug is ${gigabytes} GB, but Cargo is building, so it stays.`];
	}
	remove(root, 'target/debug');
	return [...notes, `Removed target/debug (${gigabytes} GB). The next build recompiles Rust.`];
}

function main(): number {
	const root = path.resolve(import.meta.dirname, '..');
	const [flag, ...rest] = process.argv.slice(2);
	if (rest.length > 0 || (flag !== undefined && !['--all', '--auto'].includes(flag))) {
		console.error(USAGE);
		return 2;
	}
	if (flag === '--auto') {
		try {
			for (const note of autoClean(root)) console.log(`clean: ${note}`);
		} catch (error) {
			console.error(`clean: skipped the automatic cleanup: ${String(error)}`);
		}
		return 0;
	}
	try {
		clean(root, { all: flag === '--all' });
	} catch (error) {
		console.error(`clean: ${error instanceof Error ? error.message : String(error)}`);
		return 1;
	}
	console.log(flag === '--all' ? 'Cleaned. Run bash scripts/setup.sh to restore.' : 'Cleaned.');
	return 0;
}

if (import.meta.main) {
	process.exit(main());
}
