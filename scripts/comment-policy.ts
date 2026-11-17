import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

export interface AllowEntry {
	id: string;
	reason: string;
}

export interface Finding {
	file: string;
	line: number;
	message: string;
}

export const ALLOWLIST: readonly AllowEntry[] = [
	{
		id: 'license',
		reason:
			'A required license header (SPDX, copyright, or the permission notice) has to stay in the file the license names.',
	},
	{
		id: 'lint-directive',
		reason:
			'A lint or TypeScript suppression is a directive the tool reads, and it has to carry the reason the tool or this repo requires.',
	},
	{
		id: 'shell-directive',
		reason:
			'`# allow-gnu:` and `# allow-silence:` are directives check-known-mistakes.ts reads, and each requires the reason it runs only where the marker says.',
	},
	{
		id: 'permission-fake',
		reason:
			'`// allow-permission-fake:` is a directive check-known-mistakes.ts reads, and it requires the reason the test runs unprivileged.',
	},
	{
		id: 'safety',
		reason:
			'Clippy requires `// SAFETY:` with a reason on unsafe (workspace lint undocumented_unsafe_blocks).',
	},
	{
		id: 'upstream',
		reason:
			'An upstream bug link is the pointer to a defect this repo cannot change; the link is the reason.',
	},
	{
		id: 'pin',
		reason:
			'`# pin: <why>; unpin when <trigger>` is the tag check-known-mistakes.ts requires on an exact pin.',
	},
	{
		id: 'ratchet',
		reason: '`// @ratchet-files-ignore:` with a reason is the opt-out check-file-size.mjs reads.',
	},
	{
		id: 'rustdoc-test',
		reason: 'A fenced rustdoc example is a test `cargo test --doc` runs, not an explanation.',
	},
];

const PLAIN =
	'is a comment, and a comment does not hold a constraint. Fix: delete it. If it names a constraint, encode that constraint in the API, a type, a lint rule, or a test, and put the fix in the failure text. A comment may stay only when its shape is an entry in ALLOWLIST in scripts/comment-policy.ts. Root cause: prose in code goes stale and gets copied as an excuse.';

const BARE =
	'is a directive with no reason. Fix: add the reason its form requires (` -- why` on a lint suppression, text after `// SAFETY:`, text after `# allow-gnu:` or `# allow-silence:`, `# pin: <why>; unpin when <trigger>`), or delete it and satisfy the check. Root cause: a suppression without a reason hides the constraint.';

const LICENSE =
	/SPDX-License-Identifier:|Copyright\s+\(c\)|Permission is hereby granted|licensed under the/i;
const UPSTREAM =
	/https?:\/\/(?:[\w.-]*github\.com|gitlab\.com)\/[^\s)>]+\/(?:issues|pull|pulls)\/\d+/i;
const SAFETY = /(?:^|\s)\/\/\s*SAFETY:\s+\S|\/\*\s*SAFETY:\s+\S/;
const PIN = /#\s*pin:\s+.+;\s*unpin when\s+\S/;
const SHELL_DIRECTIVE = /#\s*allow-(?:gnu|silence):\s*\S/;
const PERMISSION_FAKE = /\/\/\s*allow-permission-fake:\s*\S/;
const RATCHET = /@ratchet-files-ignore:\s*\S/;
const LINT_DIRECTIVE =
	/\b(?:oxlint-disable(?:-next-line|-line)?|eslint-disable(?:-next-line|-line)?|biome-ignore)\b/;
const TS_DIRECTIVE = /@ts-(?:expect-error|ignore|nocheck|check)\b/;

const SKIP_PREFIX = [
	'vendor/',
	'src/lib/generated/',
	'crates/abb-engine/src/app_settings/saved_settings/',
	'scripts/fixtures/',
];

const EXTENSIONS = /\.(?:rs|ts|tsx|js|mjs|cjs|css|sh|toml|ya?ml|json)$/;

export function isPolicyFile(file: string): boolean {
	return EXTENSIONS.test(file) && !SKIP_PREFIX.some((prefix) => file.startsWith(prefix));
}

interface Span {
	start: number;
	end: number;
	line: number;
	text: string;
	doc: boolean;
}

interface Mark {
	keep: boolean;
	id?: string;
	bare?: boolean;
}

function entry(id: string): Mark {
	return { keep: true, id };
}

function classify(text: string): Mark {
	if (LICENSE.test(text)) return entry('license');
	if (UPSTREAM.test(text)) return entry('upstream');
	if (SAFETY.test(text)) return entry('safety');
	if (PIN.test(text)) return entry('pin');
	if (SHELL_DIRECTIVE.test(text)) return entry('shell-directive');
	if (PERMISSION_FAKE.test(text)) return entry('permission-fake');
	if (RATCHET.test(text)) return entry('ratchet');
	if (LINT_DIRECTIVE.test(text)) {
		return /--\s*\S/.test(text) ? entry('lint-directive') : { keep: false, bare: true };
	}
	if (TS_DIRECTIVE.test(text)) {
		if (/@ts-(?:nocheck|check)\b/.test(text)) return entry('lint-directive');
		return /@ts-(?:expect-error|ignore)\s+\S/.test(text)
			? entry('lint-directive')
			: { keep: false, bare: true };
	}
	return { keep: false };
}

function lineNumber(source: string, index: number): number {
	let line = 1;
	for (let i = 0; i < index; i += 1) if (source.charCodeAt(i) === 10) line += 1;
	return line;
}

function tokenStart(source: string, index: number): boolean {
	return index === 0 || !/[\w$]/.test(source[index - 1]);
}

function skipRaw(source: string, index: number): number {
	let cursor = index;
	if (source[cursor] === 'b' || source[cursor] === 'c') cursor += 1;
	if (source[cursor] !== 'r') return index;
	cursor += 1;
	let hashes = 0;
	while (source[cursor] === '#') {
		hashes += 1;
		cursor += 1;
	}
	if (source[cursor] !== '"') return index;
	const closer = `"${'#'.repeat(hashes)}`;
	const at = source.indexOf(closer, cursor + 1);
	return at < 0 ? source.length : at + closer.length;
}

function skipQuoted(source: string, index: number, quote: string): number {
	let cursor = index + 1;
	while (cursor < source.length) {
		if (source[cursor] === '\\') {
			cursor += 2;
			continue;
		}
		if (source[cursor] === quote) return cursor + 1;
		cursor += 1;
	}
	return source.length;
}

function skipRustTick(source: string, index: number): number {
	if (source[index + 1] === '\\') {
		let cursor = index + 2;
		while (cursor < source.length && source[cursor] !== "'") {
			if (source[cursor] === '\\') cursor += 2;
			else cursor += 1;
		}
		return Math.min(source.length, cursor + 1);
	}
	if (source[index + 1] !== undefined && source[index + 2] === "'" && source[index + 1] !== "'") {
		return index + 3;
	}
	let cursor = index + 1;
	while (cursor < source.length && /[\w]/.test(source[cursor])) cursor += 1;
	return cursor;
}

function skipBlock(source: string, index: number, nested: boolean): number {
	let cursor = index + 2;
	let depth = 1;
	while (cursor < source.length) {
		if (nested && source.startsWith('/*', cursor)) {
			depth += 1;
			cursor += 2;
			continue;
		}
		if (source.startsWith('*/', cursor)) {
			depth -= 1;
			cursor += 2;
			if (depth === 0) return cursor;
			continue;
		}
		cursor += 1;
	}
	return source.length;
}

function jsxCommentEnd(source: string, index: number): number | null {
	let cursor = index + 1;
	while (cursor < source.length && /[ \t]/.test(source[cursor])) cursor += 1;
	if (!source.startsWith('/*', cursor)) return null;
	const end = skipBlock(source, cursor, false);
	let after = end;
	while (after < source.length && /[ \t]/.test(source[after])) after += 1;
	return source[after] === '}' ? after + 1 : null;
}

interface COptions {
	line: boolean;
	nested: boolean;
	rust: boolean;
	jsx: boolean;
}

function markerAt(source: string, index: number, marker: string): boolean {
	return source.startsWith(marker, index) && source[index - 1] !== '\\';
}

function pushSpan(source: string, spans: Span[], start: number, end: number, doc: boolean): void {
	spans.push({ start, end, line: lineNumber(source, start), text: source.slice(start, end), doc });
}

function skipRustLiteral(source: string, index: number): number | null {
	if (!tokenStart(source, index)) return null;
	const raw = skipRaw(source, index);
	if (raw !== index) return raw;
	if ((source[index] === 'b' || source[index] === 'c') && source[index + 1] === '"') {
		return skipQuoted(source, index + 1, '"');
	}
	if ((source[index] === 'b' || source[index] === 'c') && source[index + 1] === "'") {
		return skipRustTick(source, index + 1);
	}
	if (source[index] === "'") return skipRustTick(source, index);
	return null;
}

function scanC(source: string, options: COptions): Span[] {
	const spans: Span[] = [];
	const scan = (from: number, stop: string | null): number => {
		let index = from;
		while (index < source.length) {
			if (stop !== null && source[index] === stop) return index;
			if (options.rust) {
				const skipped = skipRustLiteral(source, index);
				if (skipped !== null) {
					index = skipped;
					continue;
				}
			}
			if (!options.rust && source[index] === '`') {
				index = skipTemplate(index);
				continue;
			}
			const quote = source[index];
			if (quote === '"' || quote === "'") {
				index = skipQuoted(source, index, quote);
				continue;
			}
			if (options.jsx && source[index] === '{') {
				const end = jsxCommentEnd(source, index);
				if (end !== null) {
					pushSpan(source, spans, index, end, false);
					index = end;
					continue;
				}
			}
			if (options.line && markerAt(source, index, '//')) {
				const end = source.indexOf('\n', index);
				const stopAt = end < 0 ? source.length : end;
				const doc = options.rust && /^\/\/[/!]/.test(source.slice(index, index + 3));
				pushSpan(source, spans, index, stopAt, doc);
				index = stopAt;
				continue;
			}
			if (markerAt(source, index, '/*')) {
				const end = skipBlock(source, index, options.nested);
				const doc = options.rust && /^\/\*[*!]/.test(source.slice(index, index + 3));
				pushSpan(source, spans, index, end, doc);
				index = end;
				continue;
			}
			index += 1;
		}
		return index;
	};
	const skipTemplate = (start: number): number => {
		let index = start + 1;
		while (index < source.length) {
			if (source[index] === '\\') {
				index += 2;
				continue;
			}
			if (source[index] === '`') return index + 1;
			if (source[index] === '$' && source[index + 1] === '{') {
				index = scan(index + 2, '}') + 1;
				continue;
			}
			index += 1;
		}
		return index;
	};
	scan(0, null);
	return spans;
}

function hashIndex(line: string, toml: boolean): number | null {
	let quote: string | null = null;
	for (let index = 0; index < line.length; index += 1) {
		const char = line[index];
		if (quote) {
			if (char === '\\' && quote !== "'") {
				index += 1;
				continue;
			}
			if (char === quote) quote = null;
			continue;
		}
		if (char === '"' || char === "'") {
			quote = char;
			continue;
		}
		if (char !== '#') continue;
		if (index === 0 && line.startsWith('#!')) return null;
		if (index > 0 && (line[index - 1] === '$' || line[index - 1] === '{')) continue;
		if (toml || index === 0 || /\s/.test(line[index - 1])) return index;
	}
	return null;
}

function heredocDelimiter(line: string): string | null {
	let quote: string | null = null;
	for (let index = 0; index < line.length - 1; index += 1) {
		const char = line[index];
		if (quote) {
			if (char === '\\' && quote !== "'") {
				index += 1;
				continue;
			}
			if (char === quote) quote = null;
			continue;
		}
		if (char === '"' || char === "'") {
			quote = char;
			continue;
		}
		if (char !== '<' || line[index + 1] !== '<') continue;
		const match = /<<-?\s*(['"]?)(\w+)\1/.exec(line.slice(index));
		return match ? match[2] : null;
	}
	return null;
}

function yamlKinds(lines: string[]): Array<'yaml' | 'run' | 'literal'> {
	const kinds: Array<'yaml' | 'run' | 'literal'> = lines.map(() => 'yaml');
	for (let index = 0; index < lines.length; index += 1) {
		const open = /^(\s*)(?:-\s*)?([\w.-]+)\s*:\s*[|>][-+]?\s*(?:#.*)?$/.exec(lines[index]);
		if (!open) continue;
		const indent = open[1].length;
		const mode = open[2] === 'run' ? 'run' : 'literal';
		for (let child = index + 1; child < lines.length; child += 1) {
			if (lines[child].trim() === '') continue;
			const childIndent = /^\s*/.exec(lines[child])?.[0].length ?? 0;
			if (childIndent <= indent) break;
			kinds[child] = mode;
		}
	}
	return kinds;
}

function tomlStringContinues(line: string, inside: boolean): { skip: boolean; inside: boolean } {
	const marks = line.match(/"""|'''/g)?.length ?? 0;
	if (inside) return { skip: true, inside: marks % 2 === 0 };
	return { skip: false, inside: marks % 2 === 1 };
}

function extractHash(source: string, kind: 'shell' | 'toml' | 'yaml'): Span[] {
	const lines = source.split('\n');
	const kinds = kind === 'yaml' ? yamlKinds(lines) : null;
	const spans: Span[] = [];
	let heredoc: string | null = null;
	let offset = 0;
	let tomlString = false;
	for (let index = 0; index < lines.length; index += 1) {
		const line = lines[index];
		const newline = index < lines.length - 1;
		const step = line.length + (newline ? 1 : 0);
		if (kind === 'toml') {
			const toml = tomlStringContinues(line, tomlString);
			tomlString = toml.inside;
			if (toml.skip) {
				offset += step;
				continue;
			}
		}
		if (heredoc) {
			if (line.trim() === heredoc) heredoc = null;
			offset += step;
			continue;
		}
		const mode = kinds?.[index] ?? 'yaml';
		const hash = mode === 'literal' ? null : hashIndex(line, kind === 'toml');
		if (hash !== null) pushSpan(source, spans, offset + hash, offset + line.length, false);
		if (kind === 'shell') heredoc = heredocDelimiter(line);
		offset += step;
	}
	return spans;
}

function cOptions(file: string): COptions {
	if (file.endsWith('.rs')) return { line: true, nested: true, rust: true, jsx: false };
	if (file.endsWith('.css')) return { line: false, nested: false, rust: false, jsx: false };
	return { line: true, nested: false, rust: false, jsx: file.endsWith('.tsx') };
}

function extract(file: string, source: string): Span[] {
	if (file.endsWith('.sh')) return extractHash(source, 'shell');
	if (file.endsWith('.toml')) return extractHash(source, 'toml');
	if (/\.ya?ml$/.test(file)) return extractHash(source, 'yaml');
	return scanC(source, cOptions(file));
}

function gap(source: string, previous: Span, next: Span): boolean {
	return /^\n[ \t]*$/.test(source.slice(previous.end, next.start));
}

function groupSpans(source: string, spans: Span[], take: (span: Span) => boolean): Span[][] {
	const groups: Span[][] = [];
	let current: Span[] = [];
	for (const span of spans) {
		if (!take(span) || (current.length > 0 && !gap(source, current[current.length - 1], span))) {
			if (current.length > 0) groups.push(current);
			current = take(span) ? [span] : [];
			continue;
		}
		current.push(span);
	}
	if (current.length > 0) groups.push(current);
	return groups;
}

function docBody(text: string): string {
	return text.replace(/^\s*\/\/[/!]?/, '').trim();
}

interface Decision {
	start: number;
	end: number;
	line: number;
	text: string;
	keep: boolean;
	id?: string;
	bare?: boolean;
}

function decideSpan(span: Span, mark: Mark): Decision {
	return {
		start: span.start,
		end: span.end,
		line: span.line,
		text: span.text,
		keep: mark.keep,
		id: mark.id,
		bare: mark.bare,
	};
}

function markDocGroup(group: Span[]): Decision[] {
	const combined = group.map((span) => span.text).join('\n');
	if (LICENSE.test(combined)) return group.map((span) => decideSpan(span, entry('license')));
	if (UPSTREAM.test(combined)) return group.map((span) => decideSpan(span, entry('upstream')));
	let inFence = false;
	return group.map((span) => {
		const body = docBody(span.text);
		const fence = body.startsWith('```');
		const keep = fence || inFence;
		if (fence) inFence = !inFence;
		return decideSpan(span, keep ? entry('rustdoc-test') : { keep: false });
	});
}

function markPlainGroup(group: Span[]): Decision[] {
	const combined = group.map((span) => span.text).join('\n');
	if (LICENSE.test(combined)) return group.map((span) => decideSpan(span, entry('license')));
	if (UPSTREAM.test(combined) && group.length <= 8) {
		return group.map((span) => decideSpan(span, entry('upstream')));
	}
	if (SAFETY.test(group[0].text)) {
		return group.map((span) => decideSpan(span, entry('safety')));
	}
	return group.map((span) => decideSpan(span, classify(span.text)));
}

export function decisions(file: string, source: string): Decision[] {
	if (!isPolicyFile(file)) return [];
	const spans = extract(file, source);
	const claimed = new Set<Span>();
	const marked: Decision[] = [];
	for (const group of groupSpans(source, spans, (span) => span.doc)) {
		for (const span of group) claimed.add(span);
		marked.push(...markDocGroup(group));
	}
	const plain = spans.filter((span) => !claimed.has(span));
	for (const group of groupSpans(source, plain, () => true)) {
		marked.push(...markPlainGroup(group));
	}
	return marked.toSorted((left, right) => left.start - right.start);
}

function messageFor(decision: Decision): string {
	const shown = decision.text.trim().replace(/\s+/g, ' ').slice(0, 80);
	return `\`${shown}\` ${decision.bare ? BARE : PLAIN}`;
}

export function commentFindings(file: string, source: string): Finding[] {
	return decisions(file, source)
		.filter((decision) => !decision.keep)
		.map((decision) => ({ file, line: decision.line, message: messageFor(decision) }));
}

export function keptComments(file: string, source: string): Array<{ line: number; id: string }> {
	return decisions(file, source)
		.filter((decision) => decision.keep && decision.id)
		.map((decision) => ({ line: decision.line, id: decision.id as string }));
}

function lineIsOnlyComment(source: string, decision: Decision): boolean {
	const lineStart = source.lastIndexOf('\n', decision.start - 1) + 1;
	const lineEnd = source.indexOf('\n', decision.end);
	const end = lineEnd < 0 ? source.length : lineEnd;
	const rest = (source.slice(lineStart, decision.start) + source.slice(decision.end, end)).trim();
	return rest === '';
}

export function stripComments(file: string, source: string): string {
	const drops = decisions(file, source)
		.filter((decision) => !decision.keep)
		.toSorted((left, right) => right.start - left.start);
	let next = source;
	for (const decision of drops) {
		let start = decision.start;
		let end = decision.end;
		if (lineIsOnlyComment(next, decision)) {
			start = next.lastIndexOf('\n', decision.start - 1) + 1;
			if (next[end] === '\n') end += 1;
		} else if (next[start - 1] === ' ') {
			start -= 1;
		}
		next = next.slice(0, start) + next.slice(end);
	}
	return next;
}

export function commentLineCount(file: string, source: string): number {
	const lines = new Set<number>();
	for (const decision of decisions(file, source)) {
		const covered = decision.text.split('\n').length;
		for (let offset = 0; offset < covered; offset += 1) lines.add(decision.line + offset);
	}
	return lines.size;
}

function tracked(repoRoot: string): string[] {
	return execFileSync('git', ['ls-files'], { cwd: repoRoot, encoding: 'utf8' })
		.split('\n')
		.filter(Boolean);
}

function languageOf(file: string): string {
	const extension = path.extname(file).slice(1);
	if (extension === 'rs') return 'rust';
	if (extension === 'ts' || extension === 'tsx') return 'typescript';
	if (extension === 'js' || extension === 'mjs' || extension === 'cjs') return 'javascript';
	if (extension === 'sh') return 'shell';
	if (extension === 'toml') return 'toml';
	if (extension === 'yml' || extension === 'yaml') return 'yaml';
	if (extension === 'css') return 'css';
	if (extension === 'json') return 'json';
	return extension;
}

function main(): number {
	const repoRoot = path.resolve(import.meta.dirname, '..');
	const mode = process.argv.includes('--count')
		? 'count'
		: process.argv.includes('--strip')
			? 'strip'
			: process.argv.includes('--kept')
				? 'kept'
				: 'check';
	const files = tracked(repoRoot).filter(isPolicyFile);
	if (mode === 'count') {
		const totals = new Map<string, number>();
		for (const file of files) {
			const count = commentLineCount(file, readFileSync(path.join(repoRoot, file), 'utf8'));
			totals.set(languageOf(file), (totals.get(languageOf(file)) ?? 0) + count);
		}
		console.log('language\tcomment_lines');
		for (const language of [...totals.keys()].toSorted()) {
			console.log(`${language}\t${totals.get(language)}`);
		}
		return 0;
	}
	if (mode === 'kept') {
		for (const allow of ALLOWLIST) console.log(`allow\t${allow.id}\t${allow.reason}`);
		for (const file of files) {
			const source = readFileSync(path.join(repoRoot, file), 'utf8');
			for (const kept of keptComments(file, source)) {
				console.log(`${file}:${kept.line}\t${kept.id}`);
			}
		}
		return 0;
	}
	if (mode === 'strip') {
		let removed = 0;
		for (const file of files) {
			const full = path.join(repoRoot, file);
			const source = readFileSync(full, 'utf8');
			const next = stripComments(file, source);
			if (next !== source) {
				removed += commentLineCount(file, source) - commentLineCount(file, next);
				writeFileSync(full, next);
			}
		}
		console.log(`removed_comment_lines\t${removed}`);
		return 0;
	}
	const findings = files.flatMap((file) =>
		commentFindings(file, readFileSync(path.join(repoRoot, file), 'utf8')),
	);
	for (const finding of findings) {
		console.error(`${finding.file}:${finding.line}: ${finding.message}`);
	}
	return findings.length === 0 ? 0 : 1;
}

if (import.meta.main) {
	process.exit(main());
}
