#!/usr/bin/env node
/**
 * Size budget for source files, so an agent can read any file whole.
 * Tokens are estimated as bytes / 4. Run with --help for modes.
 *
 * Upstream: https://github.com/dzhng/skills/tree/main/skills/engineering/ratchet-files
 * Copyright (c) 2026 David Zhang
 * SPDX-License-Identifier: MIT
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 *
 * ABB runs this copy with bun. `--against <rev>` ratchets HEAD against the
 * merge-base of that rev and HEAD. A pull-request checkout makes the index and
 * HEAD the same commit, so the staged comparison alone cannot gate the PR.
 * Staged checks are unchanged.
 */
import { execFileSync } from 'node:child_process';
import { realpathSync } from 'node:fs';
import { extname, relative } from 'node:path';
import { parseArgs } from 'node:util';
import * as languages from 'linguist-languages';

const pragma = '@ratchet-files-ignore';
const pragmaLines = 10;

const usage = `Usage:
  check-file-size.mjs --all [--max-tokens N]
      List every tracked source file over the budget, largest first.
  check-file-size.mjs [--max-tokens N] <path>...
      Ratchet the staged versions of <path>: a file over the budget fails
      unless HEAD already had it over the budget and it did not grow. A
      staged rename is measured against the file it came from.
  check-file-size.mjs --against <rev> [--max-tokens N] [path...]
      Ratchet HEAD against the merge-base of <rev> and HEAD. With no paths,
      every file changed since that merge-base is checked. A missing <rev>
      or a missing merge-base fails closed. This is the CI mode: pass the
      pull-request base. A full run can pass HEAD, which changes nothing, so
      files already over the budget do not fail just by existing.

Source is any extension GitHub Linguist assigns to a programming language
and no prose language (.md is not source). Files marked linguist-generated
or linguist-vendored in the staged .gitattributes are skipped, and so is a
file with ${pragma} in a comment in its first ${pragmaLines} lines,
followed by why splitting it would cost more than it saves. Language data
comes from the linguist-languages package, the pinned dev dependency.

The default budget is 8,000 tokens. Exit 1 means a file broke the budget.

Pre-commit install: run it on every staged file with bun or node (18.3+), e.g.
  lint-staged:  "*": "bun <path-to>/check-file-size.mjs"
  plain hook:   git diff --cached --name-only -z | xargs -0 bun <path-to>/check-file-size.mjs
Commit this script into the repository and point the hook at that copy, so
every clone runs the same check.`;

const splitRules = `Split a file before adding to it:
  - Split by concept, never by line count: each new file owns one idea and is
    named for it. No part1/part2, utils, helpers, or misc files.
  - Move whole items (types with their impls, functions with the contract
    their callers rely on) and keep the public interface where callers
    already import it.
  - Move tests beside the code they test, grouped the way the code is.
  - Commit the split on its own, before the change that needed the room. A
    split must not change behaviour; prove it with the checks the repository
    already uses for that.`;

const optOut = `Opt a file out only when every split would cost more than it saves, such
as one table or state machine that reads only as a whole. Put
${pragma} in a comment within its first ${pragmaLines} lines, followed by why:
  // ${pragma}: one opcode table; split, each half reads without the other.
The check then skips the file entirely, even as it grows, so it is a last
resort, never a way past a refusal. Tell the user each file you opt out.`;

const { values, positionals } = parseArgs({
	allowPositionals: true,
	options: {
		all: { type: 'boolean' },
		against: { type: 'string' },
		help: { type: 'boolean' },
		'max-tokens': { type: 'string', default: '8000' },
	},
});
if (values.help) {
	console.log(`${usage}\n\n${splitRules}\n\n${optOut}`);
	process.exit(0);
}
if (values.all && values.against !== undefined) {
	console.error(
		'check-file-size: --all lists every file over the budget; --against ratchets. Pass one.',
	);
	process.exit(1);
}
const maxTokens = Number(values['max-tokens']);
const maxBytes = maxTokens * 4;

const git = (args, input) =>
	execFileSync('git', args, {
		encoding: 'utf8',
		input,
		maxBuffer: 1 << 30, // A large repository lists tens of megabytes of paths.
		stdio: ['pipe', 'pipe', 'ignore'],
	});

function gitOk(args, input) {
	try {
		return git(args, input);
	} catch {
		return null;
	}
}

const rootResult = gitOk(['rev-parse', '--show-toplevel']);
if (rootResult === null) {
	console.error('check-file-size: run this inside a git repository.');
	process.exit(1);
}
const root = rootResult.trim();

const claims = new Map();
for (const { type, extensions = [] } of Object.values(languages)) {
	for (const extension of extensions) {
		claims.set(extension, [...(claims.get(extension) ?? []), type]);
	}
}
const isSource = (filePath) => {
	const types = claims.get(extname(filePath).toLowerCase()) ?? [];
	return types.includes('programming') && !types.includes('prose');
};

// Paths .gitattributes marks as generated or vendored, read from the index.
const unauthored = (paths) => {
	if (paths.length === 0) return new Set();
	const output = git(
		['check-attr', '--cached', '-z', '--stdin', 'linguist-generated', 'linguist-vendored'],
		paths.join('\0'),
	).split('\0');
	const marked = new Set();
	for (let i = 0; i + 2 < output.length; i += 3) {
		if (output[i + 2] === 'true' || output[i + 2] === 'set') marked.add(output[i]);
	}
	return marked;
};

const blobBytes = (spec) => {
	const size = gitOk(['cat-file', '-s', spec]);
	return size === null ? null : Number(size.trim());
};
// A file opts out with the pragma near its top, in whatever comment syntax
// its language uses.
const ignored = (spec) =>
	(gitOk(['cat-file', 'blob', spec]) ?? '')
		.split('\n', pragmaLines)
		.some((line) => line.includes(pragma));
const tokens = (bytes) => `~${Math.round(bytes / 4).toLocaleString('en')} tokens`;
const sources = (paths) => {
	const candidates = paths.filter(isSource);
	const skipped = unauthored(candidates);
	return candidates.filter((filePath) => !skipped.has(filePath));
};

function parseNameStatus(text) {
	const origin = new Map();
	const paths = [];
	for (const line of text.split('\n')) {
		if (line === '') continue;
		const [status, first, second] = line.split('\t');
		const kind = status?.[0];
		if (kind === 'R' || kind === 'C') {
			origin.set(second, first);
			paths.push(second);
		} else if (kind === 'A' || kind === 'M' || kind === 'T') {
			paths.push(first);
		}
	}
	return { origin, paths };
}

function collectFailures(paths, origin, currentPrefix, baselinePrefix) {
	const failures = [];
	for (const filePath of sources(paths)) {
		const current = `${currentPrefix}${filePath}`;
		const bytes = blobBytes(current);
		if (bytes === null || bytes <= maxBytes || ignored(current)) continue;
		const before = blobBytes(`${baselinePrefix}${origin.get(filePath) ?? filePath}`);
		if (before !== null && bytes <= before) continue;
		failures.push({ path: filePath, bytes, before });
	}
	return failures;
}

function whyOver(before) {
	const frozen = before !== null && before > maxBytes;
	if (frozen) {
		return {
			frozen,
			why: `already over the budget at ${tokens(before)}, so it may not grow; it does not have to get under the budget`,
		};
	}
	const was = before === null ? 'a new file' : `was ${tokens(before)}`;
	return { frozen, why: `${was}, so it must fit the budget` };
}

function reportFailures(failures, compared) {
	if (failures.length === 0) return 0;
	console.error(`Commit refused: source files over the budget of ${tokens(maxBytes)} (bytes / 4).`);
	if (compared) console.error(compared);
	for (const failure of failures) {
		const { frozen, why } = whyOver(failure.before);
		const limit = frozen ? failure.before : maxBytes;
		console.error(`  ${failure.path}: ${tokens(failure.bytes)}, ${why}.`);
		console.error(`    Cut at least ${(failure.bytes - limit).toLocaleString('en')} bytes.`);
	}
	console.error(
		`\nMake room by moving a concept into its own file, not by squeezing code to fit.\n${splitRules}\n\n${optOut}`,
	);
	return 1;
}

// Git reports the root with symlinks resolved (macOS /tmp is /private/tmp).
const inRepo = (filePath) => {
	try {
		return relative(root, realpathSync(filePath));
	} catch {
		return relative(root, filePath); // Deleted, so nothing to measure.
	}
};

if (values.all) {
	const over = sources(git(['ls-files', '-z']).split('\0').filter(Boolean))
		.map((filePath) => ({ path: filePath, bytes: blobBytes(`:${filePath}`) }))
		.filter(({ bytes }) => bytes > maxBytes)
		.sort((a, b) => b.bytes - a.bytes);
	const opted = over.filter(({ path: filePath }) => ignored(`:${filePath}`));
	const listed = over.filter((file) => !opted.includes(file));
	for (const { path: filePath, bytes } of listed) {
		console.log(`${tokens(bytes).padStart(15)}  ${filePath}`);
	}
	console.log(`Source files over ${maxTokens.toLocaleString('en')} tokens: ${listed.length}.`);
	if (opted.length > 0) {
		console.log(`Over the budget but opted out with ${pragma}: ${opted.length}.`);
	}
	process.exit(listed.length > 0 ? 1 : 0);
}

if (values.against !== undefined) {
	const resolved = gitOk([
		'rev-parse',
		'--verify',
		'--end-of-options',
		`${values.against}^{commit}`,
	]);
	if (resolved === null) {
		console.error(
			`check-file-size: base ${values.against} is not in this checkout. Fetch it and rerun; a missing base fails closed.`,
		);
		process.exit(1);
	}
	const mergeBase = gitOk(['merge-base', '--', resolved.trim(), 'HEAD']);
	if (mergeBase === null) {
		console.error(
			`check-file-size: no merge base between ${values.against} and HEAD. Fetch the base with its ancestry; a missing history fails closed.`,
		);
		process.exit(1);
	}
	const baseline = mergeBase.trim();
	const diff = parseNameStatus(git(['diff', '--find-renames', '--name-status', baseline, 'HEAD']));
	const paths = positionals.length > 0 ? positionals.map(inRepo) : diff.paths;
	const compared = `Compared with ${baseline} (merge-base of ${values.against} and HEAD).`;
	process.exit(
		reportFailures(collectFailures(paths, diff.origin, 'HEAD:', `${baseline}:`), compared),
	);
}

const staged = parseNameStatus(git(['diff', '--cached', '--find-renames', '--name-status']));
process.exit(
	reportFailures(collectFailures(positionals.map(inRepo), staged.origin, ':', 'HEAD:'), ''),
);
