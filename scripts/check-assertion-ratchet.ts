/**
 * Fails when a test file loses assertions against a base revision. A test
 * deleted in the same change as the code it imports is exempt: removing the
 * behavior removes the proof. Skipping, todo, commenting out an assertion, or
 * renaming a test off its suffix counts as a drop. Rust `assert!` and a weaker
 * matcher are known limits: docs/DECISIONS.md. Each error names the fix.
 *
 * Usage: bun scripts/check-assertion-ratchet.ts [--base <rev>] [--head <rev>]
 * The default base is the merge-base with origin/main (or main). The default
 * head is the working tree. A checkout with no base ref fails: a missing diff
 * is not a pass. Fetch the base or pass --base.
 */
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

export interface Finding {
	file: string;
	message: string;
}

interface DiffRow {
	status: string;
	from: string;
	to: string;
}

const TEST_FILE = /\.(?:test|spec)\.(?:ts|tsx)$/;
const ASSERTION = /\bexpect\s*\(|\bassert\s*\(/;
const SKIPPED = /\b(?:it|test|describe)\.(?:skipIf|skip|todo)\b|\b(?:xit|xtest|xdescribe)\s*\(/;

export function assertionCount(text: string): number {
	return executableLines(text).filter((line) => ASSERTION.test(line)).length;
}

export function skippedTestCount(text: string): number {
	return maskNonCode(text)
		.split('\n')
		.filter((line) => SKIPPED.test(line)).length;
}

export function ratchetFindings(
	root: string,
	base: string,
	head: string | undefined,
	rows: DiffRow[],
	show: (rev: string, file: string) => string | null,
): Finding[] {
	const deleted = new Set(rows.filter((row) => row.status === 'D').map((row) => row.from));
	const findings: Finding[] = [];
	for (const row of rows) {
		const finding = rowFinding(root, base, head, row, deleted, show);
		if (finding) findings.push(finding);
	}
	return findings;
}

function executableLines(text: string): string[] {
	return text
		.replace(/\/\*[\s\S]*?\*\//g, '')
		.split('\n')
		.map((line) => line.replace(/(^|[^:])\/\/.*$/, '$1'));
}

function maskNonCode(text: string): string {
	let masked = '';
	let index = 0;
	while (index < text.length) {
		if (text.startsWith('//', index)) {
			const end = text.indexOf('\n', index);
			const stop = end === -1 ? text.length : end;
			masked += blankSpan(text.slice(index, stop));
			index = stop;
			continue;
		}
		if (text.startsWith('/*', index)) {
			const end = text.indexOf('*/', index + 2);
			const stop = end === -1 ? text.length : end + 2;
			masked += blankSpan(text.slice(index, stop));
			index = stop;
			continue;
		}
		const quote = text[index];
		if (quote === '"' || quote === "'" || quote === '`') {
			const end = endOfString(text, index);
			masked += blankSpan(text.slice(index, end));
			index = end;
			continue;
		}
		masked += text[index];
		index += 1;
	}
	return masked;
}

function blankSpan(span: string): string {
	return span.replace(/[^\n]/g, ' ');
}

function endOfString(text: string, start: number): number {
	const quote = text[start];
	let index = start + 1;
	while (index < text.length) {
		if (text[index] === '\\') {
			index += 2;
			continue;
		}
		if (text[index] === quote) return index + 1;
		index += 1;
	}
	return text.length;
}

function rowFinding(
	root: string,
	base: string,
	head: string | undefined,
	row: DiffRow,
	deleted: Set<string>,
	show: (rev: string, file: string) => string | null,
): Finding | null {
	if (row.status.startsWith('R') || row.status.startsWith('C')) {
		const renamedOff = renamedOffTest(row, show(base, row.from));
		if (renamedOff) return renamedOff;
		return dropFinding(row.to, show(base, row.from), readHead(root, head, row.to, show));
	}
	if (!TEST_FILE.test(row.to) && !TEST_FILE.test(row.from)) return null;
	if (row.status === 'A') return null;
	if (row.status === 'D') {
		const before = show(base, row.from);
		if (before && assertionCount(before) > 0 && !codeDeletedWith(before, row.from, deleted)) {
			return {
				file: row.from,
				message: `assertions dropped from ${assertionCount(before)} to 0 and the code it imports is still present. Fix: keep the test, or delete that code in the same change.`,
			};
		}
		return null;
	}
	return dropFinding(row.to, show(base, row.from), readHead(root, head, row.to, show));
}

function renamedOffTest(row: DiffRow, before: string | null): Finding | null {
	if (!TEST_FILE.test(row.from) || TEST_FILE.test(row.to) || !before) return null;
	const lost = assertionCount(before);
	if (lost === 0) return null;
	return {
		file: row.from,
		message: `renamed to ${row.to}, which is not a test, dropping ${lost} assertions. Fix: keep the test suffix, or delete the code it protects in the same change.`,
	};
}

function dropFinding(file: string, before: string | null, after: string | null): Finding | null {
	if (!before || !after || !TEST_FILE.test(file)) return null;
	const from = assertionCount(before);
	const to = assertionCount(after);
	if (to < from) {
		return {
			file,
			message: `assertions dropped from ${from} to ${to}. Fix: restore the assertion, or delete the code it protects in the same change.`,
		};
	}
	const skippedFrom = skippedTestCount(before);
	const skippedTo = skippedTestCount(after);
	if (skippedTo > skippedFrom) {
		return {
			file,
			message: `skipped or todo tests increased from ${skippedFrom} to ${skippedTo}. Fix: keep the test running, or delete the code it protects in the same change.`,
		};
	}
	return null;
}

function codeDeletedWith(text: string, testPath: string, deleted: Set<string>): boolean {
	const dir = path.posix.dirname(testPath);
	for (const match of text.matchAll(/from\s+['"](\.[^'"]+)['"]/g)) {
		const base = path.posix.normalize(path.posix.join(dir, match[1]));
		for (const suffix of ['', '.ts', '.tsx', '.js', '.mjs']) {
			if (deleted.has(`${base}${suffix}`)) return true;
		}
	}
	const stem = testPath.replace(/\.(?:test|spec)\.(?:tsx?|jsx?)$/, '');
	return ['.ts', '.tsx', '.js', '.rs'].some((suffix) => deleted.has(`${stem}${suffix}`));
}

function readHead(
	root: string,
	head: string | undefined,
	file: string,
	show: (rev: string, file: string) => string | null,
): string | null {
	if (head) return show(head, file);
	const absolute = path.join(root, file);
	if (!existsSync(absolute)) return null;
	return readFileSync(absolute, 'utf8');
}

function parseNameStatus(text: string): DiffRow[] {
	return text
		.split('\n')
		.filter(Boolean)
		.map((line) => {
			const [status, from, to] = line.split('\t');
			return { status, from, to: to ?? from };
		});
}

function git(root: string, args: string[]): string {
	return execFileSync('git', args, { cwd: root, encoding: 'utf8' });
}

function gitShow(root: string, rev: string, file: string): string | null {
	try {
		return git(root, ['show', `${rev}:${file}`]);
	} catch {
		return null;
	}
}

function defaultBase(root: string): string | undefined {
	for (const name of ['origin/main', 'main']) {
		try {
			return execFileSync('git', ['merge-base', 'HEAD', name], {
				cwd: root,
				encoding: 'utf8',
				stdio: ['ignore', 'pipe', 'pipe'],
			}).trim();
		} catch {
			// The ref is missing, or this checkout is too shallow to reach it.
		}
	}
	return undefined;
}

function selectedRange(root: string, args: string[]): { base: string | undefined; head?: string } {
	let base: string | undefined;
	let head: string | undefined;
	let baseGiven = false;
	for (let index = 0; index < args.length; index++) {
		if (args[index] === '--base') {
			baseGiven = true;
			base = args[++index];
		} else if (args[index] === '--head') head = args[++index];
		else throw new Error(`Unknown argument: ${args[index]}`);
	}
	return { base: baseGiven ? base : defaultBase(root), head };
}

function main(): number {
	const root = path.resolve(import.meta.dirname, '..');
	const { base, head } = selectedRange(root, process.argv.slice(2));
	if (!base) {
		console.error(
			'No merge base with origin/main or main. The ratchet did not diff. Fix: fetch the base (`git fetch --unshallow` and `git fetch origin main:refs/remotes/origin/main`, or `git fetch --deepen=<n>` once that ref exists) or pass --base <rev>.',
		);
		return 1;
	}
	const diffArgs = ['diff', '--name-status', '-M', base];
	if (head) diffArgs.push(head);
	const findings = ratchetFindings(
		root,
		base,
		head,
		parseNameStatus(git(root, diffArgs)),
		(rev, file) => gitShow(root, rev, file),
	);
	for (const finding of findings) console.error(`${finding.file}: ${finding.message}`);
	return findings.length === 0 ? 0 : 1;
}

if (import.meta.main) {
	process.exit(main());
}
