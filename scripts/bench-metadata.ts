import { execFileSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
	closeSync,
	copyFileSync,
	fstatSync,
	mkdirSync,
	openSync,
	readSync,
	rmSync,
	statSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { type MetadataStageEvent, parseMetadataStageLine } from './dev-log-metadata';

const USAGE = `Usage: bun scripts/bench-metadata.ts [options]

Benchmarks abb-dev metadata saves on temporary copies and removes all scratch files on exit.

Options:
  --source <file>       Source audio file to copy and save. Repeatable.
                        Defaults to crates/abb-media-pipeline/tests/support/untagged.m4b.
  --local-dir <dir>     Parent folder for local scratch copies (default: OS temp dir).
  --remote-dir <dir>    Optional second scratch parent (e.g. an SMB mount) to compare.
  --runs <n>            Number of timed runs per location after 1 warmup/cold pass (default: 3).
  --edit <mode>         Edit flavor: title | series | same-len (default: title).
  --json                Print JSON summary to stdout.
  --help                Print this usage text.`;

export interface TopLevelBox {
	type: string;
	offset: number;
	size: number;
	headerLen: number;
	explicitZeroSize: boolean;
}

export interface Mp4InspectResult {
	layout: string;
	freeBytes: number;
	mdatSha256: string | null;
	boxes: TopLevelBox[];
}

interface BenchOptions {
	sources: string[];
	localDir: string;
	remoteDir?: string;
	runs: number;
	edit: 'title' | 'series' | 'same-len';
	json: boolean;
}

interface BenchRunRecord {
	location: 'local' | 'remote';
	file: string;
	fileBytes: number;
	runKind: 'cold' | 'warm';
	iteration: number;
	rc: number;
	wallMs: number;
	maxRssMb: number | null;
	layoutBefore: string;
	layoutAfter: string;
	freeBytesBefore: number;
	freeBytesAfter: number;
	audioHashMatch: boolean | null;
	stages: MetadataStageEvent[];
}

function readBoxHeader(fd: number, offset: number, fileSize: number): TopLevelBox | null {
	if (offset + 8 > fileSize) return null;
	const head = Buffer.alloc(16);
	const bytesRead = readSync(fd, head, 0, Math.min(16, fileSize - offset), offset);
	if (bytesRead < 8) return null;
	const size32 = head.readUInt32BE(0);
	const type = head.subarray(4, 8).toString('latin1');
	if (size32 === 1) {
		if (bytesRead < 16) return null;
		const size64 = Number(head.readBigUInt64BE(8));
		if (size64 < 16 || offset + size64 > fileSize) return null;
		return { type, offset, size: size64, headerLen: 16, explicitZeroSize: false };
	}
	if (size32 === 0) {
		return { type, offset, size: fileSize - offset, headerLen: 8, explicitZeroSize: true };
	}
	if (size32 < 8 || offset + size32 > fileSize) return null;
	return { type, offset, size: size32, headerLen: 8, explicitZeroSize: false };
}

function hashFileRange(fd: number, start: number, length: number): string {
	const hash = createHash('sha256');
	const chunk = Buffer.alloc(1024 * 1024);
	let remaining = length;
	let pos = start;
	while (remaining > 0) {
		const want = Math.min(chunk.length, remaining);
		const n = readSync(fd, chunk, 0, want, pos);
		if (n <= 0) break;
		hash.update(chunk.subarray(0, n));
		pos += n;
		remaining -= n;
	}
	return hash.digest('hex');
}

export function inspectMp4File(filePath: string): Mp4InspectResult | null {
	const fd = openSync(filePath, 'r');
	try {
		const fileSize = fstatSync(fd).size;
		const boxes: TopLevelBox[] = [];
		let offset = 0;
		for (let i = 0; i < 64; i += 1) {
			const box = readBoxHeader(fd, offset, fileSize);
			if (!box) break;
			boxes.push(box);
			offset += box.size;
			if (box.explicitZeroSize || offset >= fileSize) break;
		}
		if (!boxes.some((box) => box.type === 'ftyp' || box.type === 'moov')) {
			return null;
		}
		const moovIndex = boxes.findIndex((box) => box.type === 'moov');
		const mdatIndex = boxes.findIndex((box) => box.type === 'mdat');
		const layout =
			moovIndex < 0
				? 'no_moov'
				: moovIndex === boxes.length - 1
					? 'moov_end'
					: mdatIndex >= 0 && moovIndex < mdatIndex
						? 'moov_front'
						: 'moov_mid';
		const freeBytes = boxes
			.filter((box) => box.type === 'free' || box.type === 'skip')
			.reduce((sum, box) => sum + box.size, 0);
		const mdat = mdatIndex >= 0 ? boxes[mdatIndex] : undefined;
		const mdatSha256 = mdat
			? hashFileRange(fd, mdat.offset + mdat.headerLen, mdat.size - mdat.headerLen)
			: null;
		return { layout, freeBytes, mdatSha256, boxes };
	} finally {
		closeSync(fd);
	}
}

function parseArgs(argv: string[], repoRoot: string): BenchOptions | null {
	const sources: string[] = [];
	let localDir = tmpdir();
	let remoteDir: string | undefined;
	let runs = 3;
	let edit: BenchOptions['edit'] = 'title';
	let json = false;
	for (let i = 0; i < argv.length; i += 1) {
		const flag = argv[i];
		if (flag === '--help' || flag === '-h') return null;
		if (flag === '--json') {
			json = true;
			continue;
		}
		const next = argv[i + 1];
		if (!next) throw new Error(`${flag} requires a value`);
		i += 1;
		if (flag === '--source') sources.push(next);
		else if (flag === '--local-dir') localDir = next;
		else if (flag === '--remote-dir') remoteDir = next;
		else if (flag === '--runs') runs = Number.parseInt(next, 10);
		else if (flag === '--edit' && (next === 'title' || next === 'series' || next === 'same-len')) {
			edit = next;
		} else throw new Error(`Unknown option: ${flag} ${next}`);
	}
	if (!Number.isSafeInteger(runs) || runs < 1) throw new Error('--runs must be >= 1');
	if (sources.length === 0) {
		sources.push(path.join(repoRoot, 'crates/abb-media-pipeline/tests/support/untagged.m4b'));
	}
	return { sources, localDir, remoteDir, runs, edit, json };
}

function buildAbbDev(repoRoot: string): string {
	const setupOut = execFileSync('bash', ['scripts/setup.sh', '--print-env'], {
		cwd: repoRoot,
		encoding: 'utf8',
	});
	const env = { ...process.env };
	for (const line of setupOut.split('\n')) {
		const match = line.match(/^export\s+([A-Za-z_][A-Za-z0-9_]*)=(.+)$/);
		if (match) env[match[1]] = match[2].replace(/^["']|["']$/g, '');
	}
	execFileSync(
		'cargo',
		['build', '--locked', '-p', 'abb-engine', '--features', 'bundled-ffmpeg', '--bin', 'abb-dev'],
		{ cwd: repoRoot, env, stdio: 'inherit' },
	);
	const targetDir = env.CARGO_TARGET_DIR || path.join(repoRoot, 'target');
	return path.join(targetDir, 'debug', 'abb-dev');
}

function parseMaxRssMb(stderr: string): number | null {
	if (process.platform === 'darwin') {
		const match = stderr.match(/^\s*(\d+)\s+maximum resident set size/m);
		return match ? Number((Number.parseInt(match[1], 10) / (1024 * 1024)).toFixed(1)) : null;
	}
	const match = stderr.match(/Maximum resident set size \(kbytes\):\s*(\d+)/);
	return match ? Number((Number.parseInt(match[1], 10) / 1024).toFixed(1)) : null;
}

function editArgs(mode: BenchOptions['edit'], iteration: number): string[] {
	if (mode === 'series') {
		return ['--set', `series=BenchSeries${iteration}`, '--set', `series-part=${iteration}`];
	}
	if (mode === 'same-len') {
		return ['--set', `title=BenchSameLen${String(iteration).padStart(2, '0')}`];
	}
	return ['--set', `title=Bench Title Iteration ${iteration} Extended`];
}

function runSingleSave(
	abbDevBin: string,
	copyPath: string,
	stateDir: string,
	mode: BenchOptions['edit'],
	iteration: number,
): { rc: number; wallMs: number; maxRssMb: number | null; stages: MetadataStageEvent[] } {
	const cmdArgs = [
		copyPath,
		...editArgs(mode, iteration),
		'--save',
		'--json',
		'--state-dir',
		stateDir,
	];
	const wrapperArgs =
		process.platform === 'darwin' ? ['-l', abbDevBin, ...cmdArgs] : ['-v', abbDevBin, ...cmdArgs];
	const started = performance.now();
	const result = spawnSync('/usr/bin/time', wrapperArgs, {
		encoding: 'utf8',
		env: { ...process.env, RUST_LOG: 'info' },
	});
	const wallMs = Math.round(performance.now() - started);
	const stages = (result.stderr ?? '')
		.split('\n')
		.map(parseMetadataStageLine)
		.filter((stage): stage is MetadataStageEvent => stage !== null);
	return {
		rc: result.status ?? 1,
		wallMs,
		maxRssMb: parseMaxRssMb(result.stderr ?? ''),
		stages,
	};
}

function benchLocation(
	location: 'local' | 'remote',
	parentDir: string,
	options: BenchOptions,
	abbDevBin: string,
): BenchRunRecord[] {
	const scratchRoot = path.join(parentDir, `.abb-bench-${Date.now()}-${process.pid}`);
	mkdirSync(scratchRoot, { recursive: true });
	const records: BenchRunRecord[] = [];
	try {
		for (const source of options.sources) {
			const baseName = path.basename(source);
			const totalRuns = options.runs + 1;
			for (let index = 0; index < totalRuns; index += 1) {
				const copyPath = path.join(scratchRoot, `${index}-${baseName}`);
				const stateDir = path.join(scratchRoot, `state-${index}`);
				copyFileSync(source, copyPath);
				const fileBytes = statSync(copyPath).size;
				const before = inspectMp4File(copyPath);
				const run = runSingleSave(abbDevBin, copyPath, stateDir, options.edit, index);
				const after = inspectMp4File(copyPath);
				const audioHashMatch =
					before?.mdatSha256 && after?.mdatSha256 ? before.mdatSha256 === after.mdatSha256 : null;
				records.push({
					location,
					file: baseName,
					fileBytes,
					runKind: index === 0 ? 'cold' : 'warm',
					iteration: index,
					rc: run.rc,
					wallMs: run.wallMs,
					maxRssMb: run.maxRssMb,
					layoutBefore: before?.layout ?? 'non_mp4',
					layoutAfter: after?.layout ?? 'non_mp4',
					freeBytesBefore: before?.freeBytes ?? 0,
					freeBytesAfter: after?.freeBytes ?? 0,
					audioHashMatch,
					stages: run.stages,
				});
			}
		}
		return records;
	} finally {
		rmSync(scratchRoot, { recursive: true, force: true });
	}
}

function stageSummary(stages: readonly MetadataStageEvent[]): string {
	const names = [
		'metadata_copy_source',
		'metadata_mp4_tags',
		'metadata_verify_audio',
		'metadata_replace',
	];
	return names
		.flatMap((name) => {
			const found = stages.find((stage) => stage.stage === name);
			if (!found) return [];
			const tag = name.replace('metadata_', '');
			const extra = found.strategy ? `(${found.strategy})` : '';
			return [`${tag}${extra}=${found.elapsedMs ?? '?'}ms`];
		})
		.join(' ');
}

function printSummary(records: readonly BenchRunRecord[]): void {
	for (const rec of records) {
		const sizeMb = (rec.fileBytes / (1024 * 1024)).toFixed(1);
		const rss = rec.maxRssMb === null ? '—' : `${rec.maxRssMb}MB`;
		const hash = rec.audioHashMatch === null ? 'n/a' : rec.audioHashMatch ? 'ok' : 'MISMATCH';
		console.log(
			`${rec.location}\t${rec.file} (${sizeMb}MB)\t${rec.runKind}#${rec.iteration}\trc=${rec.rc}\twall=${rec.wallMs}ms\trss=${rss}\tlayout=${rec.layoutBefore}->${rec.layoutAfter}\taudio=${hash}\t${stageSummary(rec.stages)}`,
		);
	}
}

function main(): void {
	const repoRoot = path.resolve(import.meta.dirname, '..');
	const options = parseArgs(process.argv.slice(2), repoRoot);
	if (!options) {
		console.log(USAGE);
		return;
	}
	const abbDevBin = buildAbbDev(repoRoot);
	const records = benchLocation('local', options.localDir, options, abbDevBin);
	if (options.remoteDir) {
		records.push(...benchLocation('remote', options.remoteDir, options, abbDevBin));
	}
	if (options.json) console.log(JSON.stringify(records, null, 2));
	else printSummary(records);
}

if (import.meta.main) {
	try {
		main();
	} catch (error) {
		console.error(error instanceof Error ? error.message : String(error));
		process.exitCode = 1;
	}
}
