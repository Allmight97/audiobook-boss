/**
 * Fails when a test file loses assertions against a base revision. A test
 * deleted in the same change as the code it imports is exempt: removing the
 * behavior removes the proof. Each error names the fix.
 *
 * Usage: bun scripts/check-assertion-ratchet.ts [--base <rev>] [--head <rev>]
 * The default base is the merge-base with origin/main (or main). The default
 * head is the working tree.
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

export function assertionCount(text: string): number {
	return text.split('\n').filter((line) => ASSERTION.test(line)).length;
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

function rowFinding(
	root: string,
	base: string,
	head: string | undefined,
	row: DiffRow,
	deleted: Set<string>,
	show: (rev: string, file: string) => string | null,
): Finding | null {
	if (row.status.startsWith('R') || row.status.startsWith('C')) {
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

function dropFinding(file: string, before: string | null, after: string | null): Finding | null {
	if (!before || !after || !TEST_FILE.test(file)) return null;
	const from = assertionCount(before);
	const to = assertionCount(after);
	if (to >= from) return null;
	return {
		file,
		message: `assertions dropped from ${from} to ${to}. Fix: restore the assertion, or delete the code it protects in the same change.`,
	};
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

function defaultBase(root: string): string {
	for (const name of ['origin/main', 'main']) {
		try {
			return git(root, ['merge-base', 'HEAD', name]).trim();
		} catch {
			// Try the next candidate.
		}
	}
	throw new Error('No merge base with origin/main or main. Fix: pass --base <rev>.');
}

function selectedRange(root: string, args: string[]): { base: string; head: string | undefined } {
	let base: string | undefined;
	let head: string | undefined;
	for (let index = 0; index < args.length; index++) {
		if (args[index] === '--base') base = args[++index];
		else if (args[index] === '--head') head = args[++index];
		else throw new Error(`Unknown argument: ${args[index]}`);
	}
	return { base: base ?? defaultBase(root), head };
}

function main(): number {
	const root = path.resolve(import.meta.dirname, '..');
	const { base, head } = selectedRange(root, process.argv.slice(2));
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
