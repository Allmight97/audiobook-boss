import { execFileSync } from 'node:child_process';
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import {
	extractC,
	extractHtml,
	extractShell,
	extractToml,
	extractYaml,
	type Span,
} from './comment-scan';

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
			'A required license header stays in the file it names: the copyright, SPDX or permission notice, and a port provenance sentence. A narration sentence in that block does not.',
	},
	{
		id: 'lint-directive',
		reason:
			'A lint suppression is a directive the tool reads. It stays only when every rule it names is in ALLOWED_SUPPRESSIONS.',
	},
	{
		id: 'safety',
		reason:
			'Clippy requires `// SAFETY:` with a reason on unsafe (workspace lint undocumented_unsafe_blocks); the comment ends with the sentence it starts.',
	},
	{
		id: 'rustdoc-test',
		reason:
			'A fenced rustdoc example in a crate verify.sh runs with `cargo test --doc` is a test, not an explanation; a `text` or `ignore` fence, or one in another crate, runs nothing.',
	},
	{
		id: 'safety-doc',
		reason:
			'A `/// # Safety` section on an `unsafe fn` states the caller contract; clippy missing_safety_doc skips non-public functions, and no other check can hold the contract.',
	},
	{
		id: 'shellcheck',
		reason:
			'`# shellcheck shell=bash` selects the shell, and `# shellcheck disable=SC<id>` stays only when that id is in ALLOWED_SUPPRESSIONS.',
	},
];

export type SuppressionKind = 'faulty' | 'pedantic' | 'style-only';

export interface AllowedSuppression {
	readonly rule: string;
	readonly kind: SuppressionKind;
	readonly reason: string;
}

export const ALLOWED_SUPPRESSIONS: readonly AllowedSuppression[] = [
	{
		rule: 'import/default',
		kind: 'faulty',
		reason:
			'A Vite ?raw import default-exports the file text. The import resolver does not see that default.',
	},
];

const PLAIN =
	'is a comment, and a comment does not hold a constraint. Fix: delete it. If it names a constraint, encode that constraint in the API, a type, a lint rule, or a test, and put the fix in the failure text. A comment may stay only when its shape is an entry in ALLOWLIST in scripts/comment-policy.ts. Root cause: prose in code goes stale and gets copied as an excuse.';

const BARE =
	'is a directive with no reason. Fix: add the reason its form requires (` -- why` on a lint suppression, text after `// SAFETY:`, `# shellcheck disable=SC<id> # why`), or delete it and satisfy the check. Root cause: a suppression without a reason hides the constraint.';

const UNLISTED =
	'Fix: satisfy the rule, or add it to ALLOWED_SUPPRESSIONS in scripts/comment-policy.ts with a reason, and only if the rule is style-only, pedantic, or faulty. Root cause: a suppression of a correctness rule hides a bug the rule would catch.';

const NO_RULE =
	'names no rule. Fix: satisfy the check. @ts-ignore and @ts-nocheck name no rule, so they cannot be listed. Root cause: a suppression without a rule id can hide any error.';

const LICENSE =
	/SPDX-License-Identifier:|Copyright\s+(?:\(c\)|©|\d{4})|Permission is hereby granted|licensed under the/i;
const SENTENCE_END = /[.!?][)"'`]*$/;
const LINK_END = /https?:\/\/\S+$/;
const SAFETY = /(?:^|\s)\/\/\s*SAFETY:\s+\S|\/\*\s*SAFETY:\s+\S/;
const LINT_DIRECTIVE = /\b(?:(?:ox|es)lint-disable(?:-next-line|-line)?|biome-ignore)\b/;
const TS_SUPPRESSION = /@ts-(?:expect-error|ignore|nocheck)\b/;
const RUSTDOC_TEST_ATTRIBUTE =
	/^(?:rust|compile_fail|should_panic|no_run|test_harness|standalone_crate|edition\d{4}|E\d{4})$/;

export const DOCTESTED_CRATES: readonly string[] = ['abb-engine'];

const SKIP_PREFIX = [
	'vendor/',
	'src/lib/generated/',
	'crates/abb-engine/src/app_settings/saved_settings/',
	'scripts/fixtures/',
];

const EXTENSIONS = /\.(?:rs|[cm]?[jt]s|[jt]sx|css|sh|toml|ya?ml|json|html)$/;

function isShellFile(file: string): boolean {
	return file.endsWith('.sh') || file.startsWith('.githooks/');
}

export function isPolicyFile(file: string): boolean {
	const scanned = EXTENSIONS.test(file) || isShellFile(file);
	return scanned && !SKIP_PREFIX.some((prefix) => file.startsWith(prefix));
}

function extract(file: string, source: string): Span[] {
	if (isShellFile(file)) return extractShell(source);
	if (file.endsWith('.toml')) return extractToml(source);
	if (/\.ya?ml$/.test(file)) return extractYaml(source);
	if (file.endsWith('.html')) return extractHtml(source);
	return extractC(file, source);
}

interface Mark {
	keep: boolean;
	id?: string;
	bare?: boolean;
	unlisted?: readonly string[];
	noRule?: boolean;
}

const BARE_MARK: Mark = { keep: false, bare: true };
const NO_RULE_MARK: Mark = { keep: false, noRule: true };

function entry(id: string): Mark {
	return { keep: true, id };
}

function allowedRule(rule: string): boolean {
	return ALLOWED_SUPPRESSIONS.some((suppression) => suppression.rule === rule);
}

function lintRules(text: string): string[] {
	const listed = text.match(
		/\b(?:(?:ox|es)lint-disable(?:-next-line|-line)?|biome-ignore)\s+([^]*?)(?:\s--|\*\/|$)/,
	);
	const body = listed?.[1]?.trim() ?? '';
	if (body.length === 0) return [];
	return body
		.split(',')
		.map((rule) => rule.trim())
		.filter((rule) => rule.length > 0);
}

function shellcheckRules(text: string): string[] | undefined {
	const disable = text.match(/#\s*shellcheck\s+disable=([A-Za-z0-9,]+)/);
	if (!disable?.[1]) return undefined;
	return disable[1]
		.split(',')
		.map((rule) => rule.trim())
		.filter((rule) => rule.length > 0);
}

function reasoned(text: string): boolean {
	return /--\s*\S/.test(text) || /#\s*shellcheck\s+disable=\S+\s+#\s+\S/.test(text);
}

function suppressionMark(text: string, rules: readonly string[]): Mark {
	if (rules.length === 0) return NO_RULE_MARK;
	const missing = rules.filter((rule) => !allowedRule(rule));
	if (missing.length > 0) return { keep: false, unlisted: missing };
	return reasoned(text) ? entry('lint-directive') : BARE_MARK;
}

function commentBody(text: string): string {
	return text
		.replace(/^\s*(?:\/\/[/!]?|\{?\/\*[*!]?|#|<!--)/, '')
		.replace(/(?:\*\/\}?|-->)\s*$/, '')
		.trim();
}

function classify(text: string): Mark {
	if (/#\s*shellcheck\b/.test(text)) {
		if (/#\s*shellcheck\s+shell=bash\s*$/.test(text)) return entry('shellcheck');
		const rules = shellcheckRules(text);
		if (!rules) return BARE_MARK;
		const missing = rules.filter((rule) => !allowedRule(rule));
		if (missing.length > 0) return { keep: false, unlisted: missing };
		return reasoned(text) ? entry('shellcheck') : BARE_MARK;
	}
	if (LINT_DIRECTIVE.test(text)) return suppressionMark(text, lintRules(text));
	if (/@ts-check\b/.test(text) && !TS_SUPPRESSION.test(text)) return entry('lint-directive');
	if (!TS_SUPPRESSION.test(text)) return { keep: false };
	return NO_RULE_MARK;
}

function isPlain(text: string): boolean {
	const mark = classify(text);
	return !mark.keep && !mark.bare;
}

function endsSentence(span: Span): boolean {
	const body = commentBody(span.text);
	return SENTENCE_END.test(body) || LINK_END.test(body);
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

interface Decision {
	start: number;
	end: number;
	line: number;
	text: string;
	keep: boolean;
	id?: string;
	bare?: boolean;
	unlisted?: readonly string[];
	noRule?: boolean;
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
		unlisted: mark.unlisted,
		noRule: mark.noRule,
	};
}

type Claims = Array<string | null>;

function claimSentence(group: Span[], claims: Claims, at: number, id: string): void {
	claims[at] = id;
	const open = (index: number) => claims[index] === null && isPlain(group[index].text);
	for (let index = at; index + 1 < group.length && open(index + 1); index += 1) {
		if (endsSentence(group[index])) break;
		claims[index + 1] = id;
	}
}

function claimAnchors(group: Span[], claims: Claims): Claims {
	group.forEach((span, at) => {
		if (claims[at] !== null) return;
		if (!span.doc && SAFETY.test(span.text)) claimSentence(group, claims, at, 'safety');
	});
	return claims;
}

function runsDoctests(file: string): boolean {
	return DOCTESTED_CRATES.some(
		(name) => file.startsWith(`crates/${name}/src/`) && !file.startsWith(`crates/${name}/src/bin/`),
	);
}

function fenceRuns(info: string): boolean {
	return info
		.split(/[\s,]+/)
		.filter(Boolean)
		.every((attribute) => RUSTDOC_TEST_ATTRIBUTE.test(attribute));
}

function fenceClaims(file: string, group: Span[]): Claims {
	const runs = runsDoctests(file);
	let fence: boolean | null = null;
	return group.map((span) => {
		const body = commentBody(span.text);
		if (!body.startsWith('```')) return fence ? 'rustdoc-test' : null;
		const kept = fence === null ? runs && fenceRuns(body.slice(3)) : fence;
		fence = fence === null ? kept : null;
		return kept ? 'rustdoc-test' : null;
	});
}

function lineComment(span: Span): boolean {
	return /^\s*(?:\/\/|#)/.test(span.text);
}

function licenseDecisions(group: Span[], rest: (span: Span) => Mark): Decision[] {
	if (!group.every(lineComment)) {
		return group.map((span) => decideSpan(span, entry('license')));
	}
	const marked: Decision[] = [];
	let sentence: Span[] = [];
	const flush = (): void => {
		if (sentence.length === 0) return;
		const text = sentence.map((span) => commentBody(span.text)).join(' ');
		const keep = text
			.split(/(?<=[.!?])[)"'`]*\s+/)
			.filter((part) => part.trim() !== '')
			.every((part) => LICENSE.test(part) || /Ported from\b/i.test(part));
		for (const span of sentence) {
			marked.push(decideSpan(span, keep ? entry('license') : rest(span)));
		}
		sentence = [];
	};
	for (const span of group) {
		sentence.push(span);
		if (endsSentence(span)) flush();
	}
	flush();
	return marked;
}

function markGroup(group: Span[], claims: Claims, rest: (span: Span) => Mark): Decision[] {
	if (LICENSE.test(group.map((span) => span.text).join('\n'))) {
		return licenseDecisions(group, rest);
	}
	return group.map((span, index) => {
		const claim = claims[index];
		return decideSpan(span, claim === null ? rest(span) : entry(claim));
	});
}

const UNSAFE_FN =
	/^(?:\s*#\[[^\n]*\]\s*)*\s*(?:pub(?:\([^)]*\))?\s+)?(?:const\s+)?(?:async\s+)?unsafe\s+(?:extern\s+"[^"]*"\s+)?fn\b/;

function safetyDocClaims(source: string, group: Span[], claims: Claims): Claims {
	if (!UNSAFE_FN.test(source.slice(group[group.length - 1].end))) return claims;
	let open = false;
	return group.map((span, index) => {
		const body = commentBody(span.text);
		open = body === '# Safety' || (open && body !== '');
		return open ? 'safety-doc' : claims[index];
	});
}

function markDocGroup(file: string, source: string, group: Span[]): Decision[] {
	const claims = safetyDocClaims(source, group, fenceClaims(file, group));
	return markGroup(group, claimAnchors(group, claims), () => ({ keep: false }));
}

function markPlainGroup(group: Span[]): Decision[] {
	const claims = claimAnchors(
		group,
		group.map(() => null),
	);
	return markGroup(group, claims, (span) => classify(span.text));
}

function decisions(file: string, source: string): Decision[] {
	if (!isPolicyFile(file)) return [];
	const spans = extract(file, source);
	const claimed = new Set<Span>();
	const marked: Decision[] = [];
	for (const group of groupSpans(source, spans, (span) => span.doc)) {
		for (const span of group) claimed.add(span);
		marked.push(...markDocGroup(file, source, group));
	}
	const plain = spans.filter((span) => !claimed.has(span));
	for (const group of groupSpans(source, plain, () => true)) {
		marked.push(...markPlainGroup(group));
	}
	return marked.toSorted((left, right) => left.start - right.start);
}

function messageFor(decision: Decision): string {
	const shown = decision.text.trim().replace(/\s+/g, ' ').slice(0, 80);
	if (decision.noRule) return `\`${shown}\` ${NO_RULE}`;
	if (decision.unlisted && decision.unlisted.length > 0) {
		const names = decision.unlisted.map((rule) => `\`${rule}\``).join(', ');
		return `\`${shown}\` suppresses ${names}. ${UNLISTED}`;
	}
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

function commentLineCount(file: string, source: string): number {
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
	if (isShellFile(file)) return 'shell';
	const extension = path.extname(file).slice(1);
	if (extension === 'rs') return 'rust';
	if (/^[cm]?ts$|^tsx$/.test(extension)) return 'typescript';
	if (/^[cm]?js$|^jsx$/.test(extension)) return 'javascript';
	if (extension === 'yml' || extension === 'yaml') return 'yaml';
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
