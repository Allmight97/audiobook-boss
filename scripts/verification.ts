/**
 * Answers the done-gate mod's `resolve` call (`.claude/done-gate.json`): which
 * verify.sh lanes the session's changed files need and have not passed since
 * their last change. scripts/lane-paths.yml owns the path-to-lane map and
 * verify.sh owns the lane list, so this script copies neither.
 *
 * Reads `{ changed, passes: [{ command, changed }] }` on stdin and prints one
 * verify.sh command for the unmet lanes, or nothing.
 *
 * Usage: bun scripts/unmet-lanes.ts < input.json
 */
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import picomatch from 'picomatch';
import { parseDocument } from 'yaml';

export interface Pass {
	command: string;
	changed: string[];
}

export interface GateInput {
	changed: string[];
	passes: Pass[];
}

const VERIFY = /^(?:bash\s+)?(?:\.\/)?scripts\/verify\.sh((?:\s+[a-z-]+)*)$/;

export function requiredLanes(
	map: Record<string, string[]>,
	lanes: string[],
	files: string[],
): string[] {
	const known = picomatch(Object.values(map).flat(Number.POSITIVE_INFINITY), { dot: true });
	const unmapped = files.filter((file) => !known(file));
	if (unmapped.length > 0) {
		throw new Error(`Unmapped verification paths: ${unmapped.join(', ')}`);
	}
	return lanes.filter((lane) =>
		files.some((file) => picomatch(map[lane] ?? [], { dot: true })(file)),
	);
}

export function lanesRun(command: string, defaultLanes: string[]): string[] {
	const match = VERIFY.exec(command.trim());
	if (match === null) {
		return [];
	}
	const named = match[1].trim().split(/\s+/).filter(Boolean);
	return named.length === 0 ? defaultLanes : named;
}

export function unmetLanes(
	map: Record<string, string[]>,
	defaultLanes: string[],
	input: GateInput,
): string[] {
	const needed = (files: string[]) => requiredLanes(map, defaultLanes, files);
	return needed(input.changed).filter(
		(lane) =>
			!input.passes.some(
				(pass) =>
					lanesRun(pass.command, defaultLanes).includes(lane) &&
					!needed(pass.changed).includes(lane),
			),
	);
}

function defaultLanesOf(repoRoot: string): string[] {
	const help = execFileSync('bash', [path.join(repoRoot, 'scripts/verify.sh'), '--help'], {
		encoding: 'utf8',
	});
	const listed = /^Lanes: (.+)$/m.exec(help)?.[1];
	if (listed === undefined) {
		throw new Error('verify.sh --help printed no "Lanes:" line');
	}
	return listed.split(', ').filter((lane) => lane !== 'supply-chain');
}

if (import.meta.main) {
	const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
	const map = parseDocument(
		readFileSync(path.join(repoRoot, 'scripts/lane-paths.yml'), 'utf8'),
	).toJS();
	const input = JSON.parse(readFileSync(0, 'utf8')) as GateInput;
	const unmet = unmetLanes(map, defaultLanesOf(repoRoot), input);
	if (unmet.length > 0) {
		console.log(`bash scripts/verify.sh ${unmet.join(' ')}`);
	}
}
