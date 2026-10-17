/**
 * Fails when a test reads implementation source as text and asserts on it.
 * Reads of bun.lock, and reads under a temp directory, are ignored. A line
 * marked `// allow-source-text: <why the file is the contract>` is allowed.
 * Each error names the fix.
 *
 * Usage: bun scripts/check-source-text-tests.ts [--rev <rev>]
 * With no revision, checks the working tree.
 */
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';

export interface Finding {
	file: string;
	line: number;
	message: string;
}

const TEST_FILE = /\.(?:test|spec)\.(?:ts|tsx)$/;
const READ_CALL = /readFile(?:Sync)?\s*\(/;
const SOURCE_EXT = /['"`][^'"`]+\.(?:sh|bash|ts|tsx|js|jsx|mjs|cjs|rs|py)['"`]/;
const DECL = /(?:const|let)\s+(\w+)\s*=\s*([^\n]+)/g;

export function sourceTextFindings(file: string, text: string): Finding[] {
	if (!TEST_FILE.test(file)) return [];
	const temps = tempIdents(text);
	const pinsScripts = /\.scripts\b/.test(text) && /toContain\s*\(/.test(text);
	const lines = text.split('\n');
	const findings: Finding[] = [];
	for (let index = 0; index < lines.length; index++) {
		if (!READ_CALL.test(lines[index]) || allowed(lines, index)) continue;
		const argument = statementAt(lines, index);
		if (isTemp(argument, temps) || /['"`]bun\.lock['"`]/.test(argument)) continue;
		if (SOURCE_EXT.test(argument)) {
			findings.push(finding(file, index, 'implementation source'));
		} else if (pinsScripts && /package\.json['"`]/.test(argument)) {
			findings.push(finding(file, index, 'a package script string'));
		}
	}
	return findings;
}

function finding(file: string, index: number, what: string): Finding {
	return {
		file,
		line: index + 1,
		message: `reads ${what} as text. Fix: drive the behavior and assert on its output, or mark the line // allow-source-text: <why the file is the contract>.`,
	};
}

function allowed(lines: string[], index: number): boolean {
	const previous = index > 0 ? lines[index - 1] : '';
	return /allow-source-text:/.test(lines[index]) || /allow-source-text:/.test(previous);
}

function tempIdents(text: string): Set<string> {
	const temps = new Set<string>();
	const decls = [...text.matchAll(DECL)];
	let grew = true;
	while (grew) {
		grew = false;
		for (const match of decls) {
			const [, name, expr] = match;
			if (temps.has(name)) continue;
			const fromTemp = [...temps].some((ident) => new RegExp(`\\b${ident}\\b`).test(expr));
			if (/mkdtempSync|os\.tmpdir\s*\(/.test(expr) || fromTemp) {
				temps.add(name);
				grew = true;
			}
		}
	}
	return temps;
}

function isTemp(argument: string, temps: Set<string>): boolean {
	return [...temps].some((ident) => new RegExp(`\\b${ident}\\b`).test(argument));
}

function statementAt(lines: string[], index: number): string {
	const chunk = [lines[index]];
	let depth = parenDepth(lines[index]);
	for (let cursor = index + 1; cursor < lines.length && cursor < index + 8 && depth > 0; cursor++) {
		chunk.push(lines[cursor]);
		depth += parenDepth(lines[cursor]);
	}
	return chunk.join('\n');
}

function parenDepth(line: string): number {
	let depth = 0;
	for (const char of line) {
		if (char === '(') depth += 1;
		else if (char === ')') depth -= 1;
	}
	return depth;
}

function git(root: string, args: string[]): string {
	return execFileSync('git', args, { cwd: root, encoding: 'utf8' });
}

function files(root: string, rev: string | undefined): { file: string; text: string }[] {
	const list = rev
		? git(root, ['ls-tree', '-r', '--name-only', rev])
		: git(root, ['ls-files', '*.test.ts', '*.test.tsx', '*.spec.ts', '*.spec.tsx']);
	return list
		.split('\n')
		.filter((file) => TEST_FILE.test(file) && !file.startsWith('vendor/'))
		.map((file) => ({
			file,
			text: rev
				? git(root, ['show', `${rev}:${file}`])
				: readFileSync(path.join(root, file), 'utf8'),
		}));
}

function main(): number {
	const root = path.resolve(import.meta.dirname, '..');
	const args = process.argv.slice(2);
	let rev: string | undefined;
	for (let index = 0; index < args.length; index++) {
		if (args[index] === '--rev') rev = args[++index];
		else throw new Error(`Unknown argument: ${args[index]}`);
	}
	const findings = files(root, rev).flatMap(({ file, text }) => sourceTextFindings(file, text));
	for (const finding of findings) {
		console.error(`${finding.file}:${finding.line}: ${finding.message}`);
	}
	return findings.length === 0 ? 0 : 1;
}

if (import.meta.main) {
	process.exit(main());
}
