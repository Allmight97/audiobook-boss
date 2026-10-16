/**
 * Owns path-to-proof planning and local proof receipts. verify.sh owns commands.
 * Usage: bun scripts/verification.ts plan [base] | check | needed <base> | resolve
 *        bun scripts/verification.ts prepare <run-dir> <lane>
 *        bun scripts/verification.ts record <run-dir> <lane> <exit-code|skip> <seconds>
 *        bun scripts/verification.ts gate (VERIFY_NEEDS is GitHub's needs JSON)
 */
import { execFileSync, spawnSync } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import {
	existsSync,
	lstatSync,
	mkdirSync,
	readdirSync,
	readFileSync,
	readlinkSync,
	renameSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import picomatch from 'picomatch';
import { isScalar, isSeq, parseDocument } from 'yaml';

type LaneMap = Record<string, string[]>;
interface RustJob {
	os: string;
	lanes: string[];
	native: boolean;
	frontend: boolean;
}
interface Jobs {
	frontend: boolean;
	core: boolean;
	tooling: boolean;
	supplyChain: boolean;
	rust: RustJob[];
}
interface Inputs {
	fingerprint: string;
	revision: string;
	platform: string;
	arch: string;
	tools: Record<string, string>;
}
interface Receipt extends Inputs {
	lane: string;
	status: 'pass' | 'fail' | 'skip' | 'invalidated';
	durationSeconds: number;
	finishedAt: string;
	log: string;
	observedAfter?: Inputs;
}

export function requiredLanes(map: LaneMap, lanes: string[], files: string[]): string[] {
	const known = picomatch(
		[...lanes.flatMap((lane) => map[lane] ?? []), ...(map['no-lane'] ?? [])].flat(
			Number.POSITIVE_INFINITY,
		),
		{ dot: true },
	);
	const unmapped = files.filter((file) => !known(file));
	if (unmapped.length > 0) throw new Error(`Unmapped verification paths: ${unmapped.join(', ')}`);
	return lanes.filter((lane) => {
		const matches = picomatch(map[lane] ?? [], { dot: true });
		return files.some((file) => matches(file));
	});
}

export function planJobs(lanes: string[]): Jobs {
	const nativeLanes = lanes.filter((lane) => ['engine', 'media', 'host', 'decrypt'].includes(lane));
	const macLanes = [...(lanes.includes('core') ? ['core'] : []), ...nativeLanes];
	if (nativeLanes.length > 0 || lanes.includes('apple')) macLanes.push('apple');
	const rust: RustJob[] = [];
	if (nativeLanes.length > 0) {
		rust.push({
			os: 'ubuntu-latest',
			lanes: nativeLanes,
			native: true,
			frontend: lanes.includes('host'),
		});
	}
	if (macLanes.length > 0) {
		rust.push({
			os: 'macos-latest',
			lanes: macLanes,
			native: nativeLanes.length > 0 || lanes.includes('apple'),
			frontend: lanes.includes('host'),
		});
	}
	return {
		frontend: lanes.includes('frontend'),
		core: lanes.includes('core'),
		tooling: lanes.includes('tooling'),
		supplyChain: lanes.includes('supply-chain'),
		rust,
	};
}

export function receiptMatches(receipt: unknown, fingerprint: string): boolean {
	if (typeof receipt !== 'object' || receipt === null) return false;
	const value = receipt as Partial<Receipt>;
	return (
		value.status === 'pass' &&
		value.fingerprint === fingerprint &&
		value.tools !== undefined &&
		Object.values(value.tools).length > 0 &&
		Object.values(value.tools).every((tool) => tool !== 'unavailable')
	);
}

export function gateFailures(jobs: Jobs, results: Record<string, { result?: string }>): string[] {
	const expected = {
		changes: true,
		frontend: jobs.frontend,
		core: jobs.core,
		rust: jobs.rust.length > 0,
		tooling: jobs.tooling,
		'supply-chain': jobs.supplyChain,
	};
	return Object.entries(expected).flatMap(([job, needed]) => {
		const result = results[job]?.result;
		return (needed ? result === 'success' : result === 'skipped' || result === 'success')
			? []
			: [
					`${job}: expected ${needed ? 'success' : 'success or skipped'}, received ${result ?? 'missing'}`,
				];
	});
}

export function laneMap(yaml: string, lanes: string[]): LaneMap {
	const document = parseDocument(yaml);
	if (document.errors.length > 0) throw new Error(document.errors.join('\n'));
	const map = document.toJS() as Record<string, unknown>;
	for (const [lane, patterns] of Object.entries(map)) {
		if (![...lanes, 'shared', 'no-lane'].includes(lane)) throw new Error(`Unknown lane: ${lane}`);
		if (
			!Array.isArray(patterns) ||
			!patterns.flat(Number.POSITIVE_INFINITY).every((item) => typeof item === 'string')
		) {
			throw new Error(`Invalid path patterns for ${lane}`);
		}
	}
	const exemptions = document.get('no-lane');
	if (isSeq(exemptions)) {
		for (const item of exemptions.items) {
			if (!isScalar(item) || !item.comment?.trim())
				throw new Error(`no-lane needs a reason: ${String(item)}`);
		}
	}
	return Object.fromEntries(
		Object.entries(map).map(([lane, patterns]) => [
			lane,
			(patterns as string[]).flat(Number.POSITIVE_INFINITY),
		]),
	);
}

function git(root: string, args: string[]): string {
	return execFileSync('git', args, { cwd: root, encoding: 'utf8' });
}

function config(root: string): { map: LaneMap; lanes: string[] } {
	const lanes = execFileSync('bash', ['scripts/verify.sh', '--list'], {
		cwd: root,
		encoding: 'utf8',
	})
		.trim()
		.split('\n');
	return {
		lanes,
		map: laneMap(readFileSync(path.join(root, 'scripts/lane-paths.yml'), 'utf8'), lanes),
	};
}

function repositoryFiles(root: string): string[] {
	return [
		...new Set(
			git(root, ['ls-files', '-z', '--cached', '--others', '--exclude-standard'])
				.split('\0')
				.filter(Boolean),
		),
	].sort();
}

function changedFiles(root: string, base: string): string[] {
	return [
		...new Set(
			[
				...git(root, ['diff', '--name-only', '-z', base, '--']).split('\0'),
				...git(root, ['ls-files', '--others', '--exclude-standard', '-z']).split('\0'),
			].filter(Boolean),
		),
	];
}

function version(command: string, args: string[]): string {
	const result = spawnSync(command, args, { encoding: 'utf8', timeout: 15_000 });
	return result.status === 0 ? result.stdout.trim() || result.stderr.trim() : 'unavailable';
}

function toolIdentity(root: string, lane: string): Record<string, string> {
	const tools: Record<string, string> = { bun: version('bun', ['--version']) };
	if (['frontend', 'tooling', 'host'].includes(lane)) tools.node = version('node', ['--version']);
	if (['core', 'engine', 'media', 'host', 'apple', 'decrypt', 'supply-chain'].includes(lane)) {
		tools.rustc = version('rustc', ['-vV']);
		tools.cargo = version('cargo', ['--version']);
	}
	if (['engine', 'media', 'host', 'apple', 'decrypt'].includes(lane)) {
		tools.cc = version(process.env.CC ?? 'cc', ['--version']);
		tools.opus = version('pkg-config', ['--modversion', 'opus']);
		tools.ffmpeg = version(process.env.ABB_FFMPEG ?? 'ffmpeg', ['-version']);
		tools.ffprobe = version(process.env.ABB_FFPROBE ?? 'ffprobe', ['-version']);
		if (process.platform === 'darwin') tools.sdk = version('xcrun', ['--show-sdk-version']);
	}
	if (lane === 'decrypt') {
		tools.dotnet = version('dotnet', ['--version']);
		const host = /^host: (.+)$/m.exec(tools.rustc)?.[1];
		const helper = path.join(root, 'src-tauri/binaries', `abb-aaxclean-helper-${host}`);
		tools.helper =
			host && existsSync(helper)
				? createHash('sha256').update(readFileSync(helper)).digest('hex')
				: 'unavailable';
	}
	if (lane === 'core')
		tools.lizard = version('bash', ['scripts/check-rust-complexity.sh', '--version']);
	if (lane === 'tooling') {
		tools.zizmor = version('bash', ['scripts/check-workflows.sh', '--version']);
		tools.actionlint = version('actionlint', ['-version']);
		tools.shellcheck = version('shellcheck', ['--version']);
	}
	return tools;
}

export function fingerprintFiles(root: string, files: string[], context: unknown): string {
	const hash = createHash('sha256').update(JSON.stringify(context));
	for (const file of [...files].sort()) {
		const absolute = path.join(root, file);
		hash.update(`\0${file}\0`);
		if (!existsSync(absolute)) {
			hash.update('deleted');
			continue;
		}
		const stat = lstatSync(absolute);
		hash.update(String(stat.mode & 0o777));
		hash.update(stat.isSymbolicLink() ? readlinkSync(absolute) : readFileSync(absolute));
	}
	return hash.digest('hex');
}

function inputs(root: string, lane: string, map: LaneMap): Inputs {
	const matches = picomatch(map[lane] ?? [], { dot: true });
	const common = [
		'scripts/verification.ts',
		'scripts/verify.sh',
		'scripts/setup.sh',
		'scripts/lane-paths.yml',
		'.gitignore',
	];
	const files = repositoryFiles(root).filter((file) => matches(file) || common.includes(file));
	const tools = toolIdentity(root, lane);
	const environment = Object.fromEntries(
		[
			'PATH',
			'CC',
			'CFLAGS',
			'CXXFLAGS',
			'LDFLAGS',
			'RUSTFLAGS',
			'CARGO_ENCODED_RUSTFLAGS',
			'CARGO_BUILD_TARGET',
			'SDKROOT',
			'MACOSX_DEPLOYMENT_TARGET',
			'ABB_FFMPEG',
			'ABB_FFPROBE',
		].map((key) => [key, process.env[key]]),
	);
	return {
		fingerprint: fingerprintFiles(root, files, {
			lane,
			platform: process.platform,
			arch: process.arch,
			tools,
			environment,
		}),
		revision: git(root, ['rev-parse', 'HEAD']).trim(),
		platform: process.platform,
		arch: process.arch,
		tools,
	};
}

function readReceipt(file: string): unknown {
	if (!existsSync(file)) return undefined;
	try {
		return JSON.parse(readFileSync(file, 'utf8'));
	} catch {
		return undefined;
	} // An interrupted or corrupt receipt proves nothing.
}

function receiptPath(root: string, lane: string): string {
	return path.join(
		root,
		'.logs/verification/receipts',
		`${process.platform}-${process.arch}-${lane}.json`,
	);
}

function remaining(root: string, map: LaneMap, lanes: string[], files: string[]): string[] {
	const selected = requiredLanes(map, lanes, files);
	if (
		process.platform === 'darwin' &&
		selected.some((lane) => ['engine', 'media', 'host', 'decrypt'].includes(lane))
	)
		selected.push('apple');
	return selected.filter(
		(lane) =>
			lane === 'supply-chain' ||
			!receiptMatches(readReceipt(receiptPath(root, lane)), inputs(root, lane, map).fingerprint),
	);
}

function writeJson(file: string, value: unknown): void {
	mkdirSync(path.dirname(file), { recursive: true });
	const temporary = `${file}.${randomUUID()}.tmp`;
	writeFileSync(temporary, `${JSON.stringify(value, null, 2)}\n`);
	renameSync(temporary, file);
}

function record(
	root: string,
	map: LaneMap,
	run: string,
	lane: string,
	code: string,
	elapsed: string,
): void {
	const before = JSON.parse(readFileSync(path.join(run, `${lane}.inputs.json`), 'utf8')) as Inputs;
	const after = inputs(root, lane, map);
	const durationSeconds = Number(elapsed);
	if (!Number.isFinite(durationSeconds) || durationSeconds < 0)
		throw new Error('Invalid proof duration');
	const status =
		code === 'skip'
			? 'skip'
			: code !== '0'
				? 'fail'
				: before.fingerprint !== after.fingerprint
					? 'invalidated'
					: 'pass';
	const receipt: Receipt = {
		...before,
		lane,
		status,
		durationSeconds,
		finishedAt: new Date().toISOString(),
		log: path.relative(root, path.join(run, `${lane}.log`)),
		...(status === 'invalidated' ? { observedAfter: after } : {}),
	};
	writeJson(path.join(run, `${lane}.json`), receipt);
	writeJson(receiptPath(root, lane), receipt);
	if (status === 'invalidated')
		throw new Error(`${lane}: inputs changed during verification; rerun this lane`);
}

function emitPlan(root: string, map: LaneMap, lanes: string[], base?: string): void {
	requiredLanes(map, lanes, repositoryFiles(root));
	const selected =
		base === '--advisories'
			? ['supply-chain']
			: base
				? requiredLanes(map, lanes, changedFiles(root, base))
				: lanes;
	const jobs = planJobs(selected);
	const output = {
		frontend: String(jobs.frontend),
		core: String(jobs.core),
		tooling: String(jobs.tooling),
		supply_chain: String(jobs.supplyChain),
		rust_needed: String(jobs.rust.length > 0),
		matrix: JSON.stringify({ include: jobs.rust }),
		plan: JSON.stringify(jobs),
	};
	if (process.env.GITHUB_OUTPUT) {
		writeFileSync(
			process.env.GITHUB_OUTPUT,
			Object.entries(output)
				.map(([key, value]) => `${key}=${value}\n`)
				.join(''),
			{ flag: 'a' },
		);
	}
	console.log(JSON.stringify({ lanes: selected, jobs }, null, 2));
}

function summarize(run: string): void {
	const receipts = readdirSync(run)
		.filter(
			(file) => file.endsWith('.json') && !file.endsWith('.inputs.json') && file !== 'results.json',
		)
		.map((file) => JSON.parse(readFileSync(path.join(run, file), 'utf8')) as Receipt);
	writeJson(path.join(run, 'results.json'), receipts);
	if (process.env.GITHUB_STEP_SUMMARY)
		writeFileSync(
			process.env.GITHUB_STEP_SUMMARY,
			receipts
				.map(
					(receipt) =>
						`- ${receipt.lane}: **${receipt.status}**, ${receipt.durationSeconds}s, inputs \`${receipt.fingerprint}\`\n`,
				)
				.join(''),
			{ flag: 'a' },
		);
}

function resolveForHarness(root: string, map: LaneMap, lanes: string[]): void {
	const input = JSON.parse(readFileSync(0, 'utf8')) as { changed?: unknown };
	if (!Array.isArray(input.changed) || !input.changed.every((file) => typeof file === 'string'))
		throw new Error('resolve requires changed paths');
	const needed = remaining(root, map, lanes, input.changed);
	if (needed.length > 0) console.log(`bash scripts/verify.sh ${needed.join(' ')}`);
}

function main(): void {
	const root = path.resolve(import.meta.dirname, '..');
	const { map, lanes } = config(root);
	const [command, first, second, third, fourth] = process.argv.slice(2);
	switch (command) {
		case 'check':
			requiredLanes(map, lanes, repositoryFiles(root));
			return;
		case 'plan':
			emitPlan(root, map, lanes, first);
			return;
		case 'needed':
			if (!first) throw new Error('needed requires a base revision');
			console.log(remaining(root, map, lanes, changedFiles(root, first)).join('\n'));
			return;
		case 'resolve':
			resolveForHarness(root, map, lanes);
			return;
		case 'prepare':
			if (!first || !lanes.includes(second))
				throw new Error('prepare requires a run directory and known lane');
			writeJson(path.join(first, `${second}.inputs.json`), inputs(root, second, map));
			return;
		case 'record':
			if (!first || !lanes.includes(second) || !third || !fourth)
				throw new Error('record requires a run directory, known lane, exit code, and seconds');
			record(root, map, first, second, third, fourth);
			return;
		case 'summarize':
			if (!first) throw new Error('summarize requires a run directory');
			summarize(first);
			return;
		case 'gate': {
			const needs = JSON.parse(process.env.VERIFY_NEEDS ?? '') as Record<
				string,
				{ result?: string; outputs?: { plan?: string } }
			>;
			if (needs.changes?.result !== 'success')
				throw new Error('Verification planning did not succeed');
			const failures = gateFailures(JSON.parse(needs.changes.outputs?.plan ?? '') as Jobs, needs);
			if (failures.length > 0) throw new Error(failures.join('\n'));
			console.log('Every selected verification job passed.');
			return;
		}
		default:
			throw new Error(`Unknown verification command: ${command ?? '(missing)'}`);
	}
}

if (import.meta.main) {
	try {
		main();
	} catch (error) {
		console.error(error instanceof Error ? error.message : String(error));
		process.exitCode = 1;
	}
}
