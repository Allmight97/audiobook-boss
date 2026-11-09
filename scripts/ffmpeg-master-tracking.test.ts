import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';
import {
	type Library,
	type TrackingReport,
	commitsTouching,
	daysBehind,
	enabledBuild,
	issueToUpdate,
	librariesFromBuildRs,
	n91Releases,
	readPin,
	renderIssue,
	trackingPathspecs,
} from './ffmpeg-master-tracking';

const TITLE = 'Weekly FFmpeg master tracking';
const repoRoot = path.resolve(import.meta.dirname, '..');
const repos: string[] = [];

afterEach(() => {
	for (const repo of repos.splice(0)) rmSync(repo, { recursive: true, force: true });
});

describe('ffmpeg master tracking', () => {
	it('reads a commit sha and rejects anything else', () => {
		expect(readPin(`${'a'.repeat(40)}\n`)).toBe('a'.repeat(40));
		expect(() => readPin('5048d707')).toThrow(/commit sha/);
	});

	it('counts whole days between committer dates', () => {
		expect(daysBehind('2026-11-04T20:18:17Z', '2026-11-09T11:12:28Z')).toBe(4);
		expect(daysBehind('2026-11-04T00:00:00Z', '2026-11-05T00:00:00Z')).toBe(1);
		expect(daysBehind('2026-11-04T00:00:00Z', '2026-11-04T23:59:59Z')).toBe(0);
		expect(daysBehind('2026-11-09T00:00:00Z', '2026-11-04T00:00:00Z')).toBe(0);
		expect(() => daysBehind('not-a-date', '2026-11-04T00:00:00Z')).toThrow(/did not parse/);
	});

	it('derives the enabled libraries and pathspecs from the vendored build', () => {
		const enabled = enabledBuild(
			readFileSync(path.join(repoRoot, 'crates/abb-engine/Cargo.toml'), 'utf8'),
			readFileSync(path.join(repoRoot, 'vendor/ffmpeg-next-9.0.0/Cargo.toml'), 'utf8'),
			librariesFromBuildRs(
				readFileSync(path.join(repoRoot, 'vendor/ffmpeg-sys-next-9.0.1/build.rs'), 'utf8'),
			),
		);
		expect(enabled.compiles).toBe(true);
		expect(enabled.libraries).toEqual(['avcodec', 'avformat', 'avutil', 'swresample']);
		expect(enabled.externals).toEqual(['build-audiotoolbox', 'build-lib-opus']);
		expect(trackingPathspecs(enabled.libraries, enabled.externals)).toEqual([
			'libavcodec/aac*',
			':(glob)**/*opus*',
			'libavformat/mov*',
			':(glob)**/*audiotoolbox*',
			'libavutil',
			'libswresample',
		]);
	});

	it('adds a library the bundled build enables and leaves a disabled one out', () => {
		const next = `
[features]
codec = ["ffmpeg-sys-next/avcodec"]
format = ["ffmpeg-sys-next/avformat", "codec"]
software-scaling = ["ffmpeg-sys-next/swscale", "codec"]
software-resampling = ["ffmpeg-sys-next/swresample"]
build = ["static", "ffmpeg-sys-next/build"]
static = ["ffmpeg-sys-next/static"]
`;
		const engine = `
ffmpeg-next = { version = "9.0.0", default-features = false, features = ["format", "software-scaling"] }
[features]
bundled-ffmpeg = [
    "ffmpeg-next/build",
    "ffmpeg-sys-next/build-lib-opus",
]
`;
		const libraries: Library[] = [
			{ name: 'avcodec', isFeature: true },
			{ name: 'avformat', isFeature: true },
			{ name: 'avutil', isFeature: false },
			{ name: 'swresample', isFeature: true },
			{ name: 'swscale', isFeature: true },
		];
		const enabled = enabledBuild(engine, next, libraries);
		expect(enabled.libraries).toEqual(['avcodec', 'avformat', 'avutil', 'swscale']);
		expect(enabled.externals).toEqual(['build-lib-opus']);
		expect(trackingPathspecs(enabled.libraries, enabled.externals)).not.toContain('libswresample');
		expect(trackingPathspecs(enabled.libraries, enabled.externals)).toContain('libswscale');
	});

	it('treats n9.1 and n9.1.N as the release and ignores the dev tag', () => {
		expect(
			n91Releases([
				{ ref: 'refs/tags/n9.1-dev', sha: 'a'.repeat(40), type: 'tag' },
				{ ref: 'refs/tags/n9.10', sha: 'b'.repeat(40), type: 'commit' },
				{ ref: 'refs/tags/n9.1', sha: 'c'.repeat(40), type: 'tag' },
				{ ref: 'refs/tags/n9.1.2', sha: 'd'.repeat(40), type: 'commit' },
			]).map((release) => release.name),
		).toEqual(['n9.1', 'n9.1.2']);
	});

	it('edits only a bot issue with the exact title and label', () => {
		const human = issue(1, 'OPEN', 'someone');
		const wrongTitle = issue(2, 'OPEN', 'github-actions[bot]', 'Something else');
		const unlabeled = issue(3, 'OPEN', 'github-actions[bot]', TITLE, []);
		const closed = issue(4, 'CLOSED', 'github-actions[bot]');
		const open = issue(5, 'OPEN', 'github-actions[bot]');
		expect(issueToUpdate([])).toBeNull();
		expect(issueToUpdate([human, wrongTitle, unlabeled])).toBeNull();
		expect(issueToUpdate([human, closed, open])?.number).toBe(5);
		expect(issueToUpdate([human, { ...closed, state: 'closed' }])?.number).toBe(4);
	});

	it('lists only commits on the derived paths', () => {
		const repo = initRepo();
		const root = commit(repo, 'README', '2026-11-01T00:00:00Z');
		commit(repo, 'notes.txt', '2026-11-02T00:00:00Z');
		const aac = commit(repo, 'libavcodec/aacenc.c', '2026-11-03T00:00:00Z');
		const aacDir = commit(repo, 'libavcodec/aac/aacdec.c', '2026-11-04T00:00:00Z');
		const opus = commit(repo, 'libavcodec/opusdec.c', '2026-11-05T00:00:00Z');
		const mov = commit(repo, 'libavformat/movenc.c', '2026-11-06T00:00:00Z');
		const util = commit(repo, 'libavutil/frame.c', '2026-11-07T00:00:00Z');
		const paths = ['libavcodec/aac*', ':(glob)**/*opus*', 'libavformat/mov*', 'libavutil'];
		const touched = commitsTouching(repo, root, 'HEAD', paths, 40);
		expect(touched.total).toBe(5);
		expect(touched.commits.map((entry) => entry.sha)).toEqual([util, mov, opus, aacDir, aac]);
		expect(touched.commits.map((entry) => entry.subject)).not.toContain('notes.txt');
		expect(commitsTouching(repo, root, 'HEAD', [], 40)).toEqual({ total: 0, commits: [] });
		expect(commitsTouching(repo, root, 'HEAD', paths, 1).commits).toHaveLength(1);
		expect(commitsTouching(repo, root, 'HEAD', paths, 1).total).toBe(5);
	});

	it('renders the counts, the shas, and the n9.1 flag', () => {
		const absent = renderIssue(report({ releases: [] }));
		expect(absent.startsWith('**n9.1:** not tagged.')).toBe(true);
		expect(absent).not.toContain('<!--');
		expect(absent).toContain('**4 commits behind**, **2 days behind**');
		expect(absent).toContain('Compare API `ahead_by` 4, `behind_by` 0.');
		expect(absent).toContain(`\`${'a'.repeat(40)}\``);
		expect(absent).toContain(`\`${'b'.repeat(40)}\``);
		expect(absent).toContain('**n9.1:** not tagged.');
		expect(absent).toContain('`n9.1-dev` is not a release.');
		expect(absent).not.toContain('TAGGED');
		expect(absent).toContain('`aacenc: restore the coefficient`');
		expect(absent).toContain('does not move the pin');
		expect(absent).toContain(
			'[this run](https://github.com/Allmight97/audiobook-boss/actions/runs/1)',
		);

		const tagged = renderIssue(report({ releases: [{ name: 'n9.1', sha: 'c'.repeat(40) }] }));
		expect(tagged.startsWith('**n9.1: TAGGED**')).toBe(true);
		expect(tagged).toContain(`\`n9.1\` at \`${'c'.repeat(40)}\``);

		const current = renderIssue(report({ masterAhead: 0, pinNotOnMaster: 0, daysBehind: 0 }));
		expect(current).toContain('**Up to date** with FFmpeg master.');

		const diverged = renderIssue(report({ masterAhead: 3, pinNotOnMaster: 1 }));
		expect(diverged).toContain('The pin also has 1 commit absent from master.');

		const capped = renderIssue(report({ commitsTotal: 5 }));
		expect(capped).toContain('Showing 1 of 5, newest first.');
	});
});

function issue(
	number: number,
	state: string,
	author: string,
	title = TITLE,
	labels: string[] = ['ffmpeg-master-tracking'],
) {
	return { number, state, title, author, labels };
}

function report(overrides: Partial<TrackingReport>): TrackingReport {
	return {
		pin: 'a'.repeat(40),
		pinDate: '2026-11-01T00:00:00Z',
		head: 'b'.repeat(40),
		headDate: '2026-11-03T00:00:00Z',
		masterAhead: 4,
		pinNotOnMaster: 0,
		daysBehind: 2,
		compareUrl: 'https://github.com/FFmpeg/FFmpeg/compare/pin...master',
		releases: [],
		pathspecs: ['libavcodec/aac*'],
		commits: [
			{
				sha: 'd'.repeat(40),
				date: '2026-11-02T00:00:00Z',
				subject: 'aacenc: restore the coefficient',
			},
		],
		commitsTotal: 1,
		runUrl: 'https://github.com/Allmight97/audiobook-boss/actions/runs/1',
		...overrides,
	};
}

function initRepo(): string {
	const repo = mkdtempSync(path.join(tmpdir(), 'abb-ffmpeg-log-'));
	repos.push(repo);
	git(repo, ['init', '-b', 'main']);
	return repo;
}

function commit(repo: string, file: string, date: string): string {
	const target = path.join(repo, file);
	mkdirSync(path.dirname(target), { recursive: true });
	writeFileSync(target, file);
	git(repo, ['add', file]);
	git(repo, ['commit', '-m', file], date);
	return git(repo, ['rev-parse', 'HEAD']).trim();
}

function git(repo: string, args: string[], date?: string): string {
	return execFileSync(
		'git',
		['-C', repo, '-c', 'user.name=ABB Test', '-c', 'user.email=abb-test@example.com', ...args],
		{
			encoding: 'utf8',
			env: {
				...process.env,
				GIT_AUTHOR_DATE: date,
				GIT_COMMITTER_DATE: date,
			},
		},
	);
}
