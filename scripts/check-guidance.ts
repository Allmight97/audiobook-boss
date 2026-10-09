/**
 * Fails when agent guidance copies what a script or workflow owns, names a
 * package script that does not exist, or points at a repo path that does not
 * exist. Guidance names the owner (a verify.sh lane, a script) and never its
 * flags, so a changed command has one place to change.
 *
 * Usage: bun scripts/check-guidance.ts
 */
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

export interface Finding {
	file: string;
	line: number;
	message: string;
}

export interface GuidanceContext {
	packageScripts: Set<string>;
	trackedTopLevel: Set<string>;
	pathExists: (repoRelativePath: string) => boolean;
}

const GUIDANCE_FILE =
	/(^|\/)(AGENTS|CLAUDE|REVIEW)\.md$|^README\.md$|^\.agents\/.*\.md$|^docs\/.*\.md$/;
const COMMAND_COPY = /\bcargo (test|clippy|fmt|run|build|check|audit|deny)\b|--features\b/;
const BUN_RUN = /^bun run ([\w:.-]+)/;
const SCRIPT_NAME = /^[a-z][a-z0-9-]*(?::[a-z0-9-]+)+$/;
const SINGLE_OWNER_FACTS: { pattern: RegExp; owner: string }[] = [
	{ pattern: /\bready_for_review\b|\bauto_merge_enabled\b/, owner: '.github/workflows/ci.yml' },
];

export function isGuidanceFile(repoRelativePath: string): boolean {
	return GUIDANCE_FILE.test(repoRelativePath);
}

function codeSpans(line: string): string[] {
	return [...line.matchAll(/`([^`]+)`/g)].map((match) => match[1]);
}

function looksLikeRepoPath(span: string): boolean {
	return (
		span.includes('/') &&
		!/[\s*<>{}$~]|^[a-z]+:\/\/|^@|^-/.test(span) &&
		!span.startsWith('/') &&
		(/\.[a-z0-9]+$/i.test(span) || span.endsWith('/'))
	);
}

function pathFinding(file: string, span: string, context: GuidanceContext): string | null {
	const target = span.replace(/:\d+(-\d+)?$/, '').replace(/\/$/, '');
	const fromFile = path.posix.normalize(path.posix.join(path.posix.dirname(file), target));
	const topLevel = target.split('/')[0];
	if (!context.trackedTopLevel.has(topLevel) && !existsAt(fromFile, context)) {
		return null;
	}
	if (context.pathExists(target) || existsAt(fromFile, context)) {
		return null;
	}
	return `\`${span}\` names a path that does not exist. Fix: point at the real file, or drop the path.`;
}

function existsAt(repoRelativePath: string, context: GuidanceContext): boolean {
	return !repoRelativePath.startsWith('..') && context.pathExists(repoRelativePath);
}

function lineFindings(
	file: string,
	line: string,
	inFence: boolean,
	context: GuidanceContext,
): string[] {
	const messages: string[] = [];
	const spans = inFence ? [line] : codeSpans(line);
	for (const span of spans) {
		if (COMMAND_COPY.test(span)) {
			messages.push(
				`\`${span.trim()}\` copies a command that a script owns. Fix: name the lane (\`bash scripts/verify.sh <lane>\`) or the script (\`bash scripts/abb-dev.sh\`), not its flags.`,
			);
		}
		const scriptName = span.trim().match(BUN_RUN)?.[1] ?? span.trim().match(SCRIPT_NAME)?.[0];
		if (scriptName && !context.packageScripts.has(scriptName)) {
			messages.push(
				`\`${scriptName}\` is not a package.json script. Fix: use a script that exists.`,
			);
		}
		if (!inFence && looksLikeRepoPath(span)) {
			const message = pathFinding(file, span, context);
			if (message) {
				messages.push(message);
			}
		}
	}
	for (const fact of SINGLE_OWNER_FACTS) {
		if (fact.pattern.test(line)) {
			messages.push(`Restates a fact owned by \`${fact.owner}\`. Fix: point at that file instead.`);
		}
	}
	return messages;
}

export function checkGuidance(file: string, text: string, context: GuidanceContext): Finding[] {
	const findings: Finding[] = [];
	let inFence = false;
	text.split('\n').forEach((line, index) => {
		if (/^\s*```/.test(line)) {
			inFence = !inFence;
			return;
		}
		for (const message of lineFindings(file, line, inFence, context)) {
			findings.push({ file, line: index + 1, message });
		}
	});
	return findings;
}

function main(): number {
	const repoRoot = path.resolve(import.meta.dirname, '..');
	const tracked = execFileSync('git', ['ls-files'], { cwd: repoRoot, encoding: 'utf8' })
		.split('\n')
		.filter(Boolean);
	const packageJson = JSON.parse(readFileSync(path.join(repoRoot, 'package.json'), 'utf8'));
	const context: GuidanceContext = {
		packageScripts: new Set(Object.keys(packageJson.scripts)),
		trackedTopLevel: new Set(tracked.map((file) => file.split('/')[0])),
		pathExists: (repoRelativePath) => existsSync(path.join(repoRoot, repoRelativePath)),
	};
	const findings = tracked
		.filter(isGuidanceFile)
		.flatMap((file) =>
			checkGuidance(file, readFileSync(path.join(repoRoot, file), 'utf8'), context),
		);
	for (const finding of findings) {
		console.error(`${finding.file}:${finding.line}: ${finding.message}`);
	}
	return findings.length === 0 ? 0 : 1;
}

if (import.meta.main) {
	process.exit(main());
}
