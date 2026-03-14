import { spawnSync } from 'node:child_process';
import {
	existsSync,
	lstatSync,
	mkdirSync,
	readFileSync,
	readlinkSync,
	symlinkSync,
	unlinkSync,
} from 'node:fs';
import path from 'node:path';

interface PackageJson {
	name: string;
}

interface TauriConfig {
	productName: string;
}

interface BundlePaths {
	productName: string;
	bundleDir: string;
	canonicalAppPath: string;
	executablePath: string;
	applicationsLinkPath: string;
}

type ApplicationsLinkOutcome = 'created' | 'updated' | 'skipped';

interface LinkFsOps {
	lstatSync: typeof lstatSync;
	mkdirSync: typeof mkdirSync;
	readlinkSync: typeof readlinkSync;
	symlinkSync: typeof symlinkSync;
	unlinkSync: typeof unlinkSync;
}

const defaultLinkFsOps: LinkFsOps = {
	lstatSync,
	mkdirSync,
	readlinkSync,
	symlinkSync,
	unlinkSync,
};

function readJson<T>(filePath: string): T {
	return JSON.parse(readFileSync(filePath, 'utf8')) as T;
}

function mergeFeatureList(value: string, requiredFeature: string): string {
	const normalized = value
		.split(',')
		.map((entry) => entry.trim())
		.filter((entry) => entry.length > 0);
	if (normalized.includes(requiredFeature)) {
		return normalized.join(',');
	}
	return [...normalized, requiredFeature].join(',');
}

export function ensureFeatureArg(args: string[], flag: string, requiredFeature: string): string[] {
	const [tauriArgs, trailingArgs] = splitArgsAtBoundary(args);
	const nextTauriArgs = [...tauriArgs];
	let foundFlag = false;

	for (let index = 0; index < nextTauriArgs.length; index += 1) {
		const token = nextTauriArgs[index];

		if (token === flag || (flag === '--features' && token === '-f')) {
			foundFlag = true;
			const valueIndex = index + 1;
			const existingValue = nextTauriArgs[valueIndex];
			if (!existingValue || existingValue.startsWith('-')) {
				nextTauriArgs.splice(valueIndex, 0, requiredFeature);
				index = valueIndex;
				continue;
			}

			nextTauriArgs[valueIndex] = mergeFeatureList(existingValue, requiredFeature);
			index = valueIndex;
			continue;
		}

		if (flag === '--features' && token.startsWith('--features=')) {
			foundFlag = true;
			const existingValue = token.slice('--features='.length);
			const mergedValue = mergeFeatureList(existingValue, requiredFeature);
			nextTauriArgs[index] = `--features=${mergedValue}`;
		}
	}

	if (!foundFlag) {
		nextTauriArgs.push(flag, requiredFeature);
	}

	return [...nextTauriArgs, ...trailingArgs];
}

function splitArgsAtBoundary(args: string[]): [string[], string[]] {
	const boundaryIndex = args.indexOf('--');
	if (boundaryIndex === -1) {
		return [args, []];
	}
	return [args.slice(0, boundaryIndex), args.slice(boundaryIndex)];
}

export function assertSupportedMacOsHost(platform = process.platform, arch = process.arch): void {
	if (platform !== 'darwin') {
		return;
	}

	if (arch !== 'arm64') {
		throw new Error(
			`AudioBook Boss supports only native Apple Silicon macOS hosts. Refusing host architecture '${arch}'.`,
		);
	}
}

export function resolveMacOsBundlePaths(
	repoRoot: string,
	applicationsDir = '/Applications',
): BundlePaths {
	const packageJson = readJson<PackageJson>(path.join(repoRoot, 'package.json'));
	const tauriConfig = readJson<TauriConfig>(path.join(repoRoot, 'src-tauri/tauri.conf.json'));
	const bundleDir = path.join(repoRoot, 'target/release/bundle/macos');
	const canonicalAppPath = path.join(bundleDir, `${tauriConfig.productName}.app`);

	return {
		productName: tauriConfig.productName,
		bundleDir,
		canonicalAppPath,
		executablePath: path.join(canonicalAppPath, 'Contents', 'MacOS', packageJson.name),
		applicationsLinkPath: path.join(applicationsDir, `${tauriConfig.productName}.app`),
	};
}

export function buildTauriApp(repoRoot: string, buildArgs: string[]): void {
	const tauriArgs = ensureFeatureArg(buildArgs, '--features', 'bundled-ffmpeg');
	const result = spawnSync('bun', ['run', 'tauri', 'build', ...tauriArgs], {
		cwd: repoRoot,
		stdio: 'inherit',
	});

	if (typeof result.status === 'number' && result.status !== 0) {
		process.exit(result.status);
	}

	if (result.error) {
		throw result.error;
	}
}

export function verifyMacOsBundle(paths: BundlePaths): void {
	if (!existsSync(paths.canonicalAppPath)) {
		throw new Error(`Expected canonical app bundle at ${paths.canonicalAppPath}`);
	}

	if (!existsSync(paths.executablePath)) {
		throw new Error(`Expected app executable at ${paths.executablePath}`);
	}

	verifyMacOsExecutableArchitecture(paths.executablePath);

	const result = spawnSync('otool', ['-L', paths.executablePath], {
		encoding: 'utf8',
	});

	if (result.status !== 0) {
		throw new Error(result.stderr.trim() || `otool failed for ${paths.executablePath}`);
	}

	const forbiddenMatches = findForbiddenLinkedLibraries(result.stdout);

	if (forbiddenMatches.length > 0) {
		throw new Error(
			[
				`Packaged app still links external libraries: ${paths.executablePath}`,
				...forbiddenMatches,
			].join('\n'),
		);
	}
}

function verifyMacOsExecutableArchitecture(executablePath: string): void {
	const result = spawnSync('lipo', ['-archs', executablePath], {
		encoding: 'utf8',
	});

	if (result.status !== 0) {
		throw new Error(result.stderr.trim() || `lipo failed for ${executablePath}`);
	}

	const unsupportedArchitectures = findUnsupportedMacOsArchitectures(result.stdout);
	if (unsupportedArchitectures.length > 0) {
		throw new Error(
			[
				`Packaged app contains unsupported macOS architectures: ${executablePath}`,
				...unsupportedArchitectures,
			].join('\n'),
		);
	}
}

export function findForbiddenLinkedLibraries(otoolOutput: string): string[] {
	return otoolOutput
		.split('\n')
		.map((line) => line.trim())
		.filter((line) => line.startsWith('/opt/homebrew/') || line.startsWith('/usr/local/'));
}

export function findUnsupportedMacOsArchitectures(lipoOutput: string): string[] {
	return lipoOutput
		.trim()
		.split(/\s+/)
		.filter((arch) => arch.length > 0)
		.filter((arch) => arch !== 'arm64' && arch !== 'arm64e');
}

export function refreshApplicationsLink(
	paths: BundlePaths,
	fsOps: LinkFsOps = defaultLinkFsOps,
): ApplicationsLinkOutcome {
	try {
		fsOps.mkdirSync(path.dirname(paths.applicationsLinkPath), { recursive: true });
	} catch (error) {
		if (isPermissionDeniedError(error)) {
			return 'skipped';
		}
		throw error;
	}

	if (!existsSync(paths.canonicalAppPath)) {
		throw new Error(`Expected canonical app bundle at ${paths.canonicalAppPath}`);
	}

	const existingLinkStats = readLinkStats(paths.applicationsLinkPath, fsOps.lstatSync);
	if (existingLinkStats === 'permission-denied') {
		return 'skipped';
	}
	if (existingLinkStats === null) {
		try {
			fsOps.symlinkSync(paths.canonicalAppPath, paths.applicationsLinkPath, 'dir');
		} catch (error) {
			if (isPermissionDeniedError(error)) {
				return 'skipped';
			}
			throw error;
		}
		return 'created';
	}

	if (!existingLinkStats.isSymbolicLink()) {
		return 'skipped';
	}

	const currentTarget = path.resolve(
		path.dirname(paths.applicationsLinkPath),
		fsOps.readlinkSync(paths.applicationsLinkPath),
	);
	if (currentTarget === paths.canonicalAppPath) {
		return 'skipped';
	}

	try {
		fsOps.unlinkSync(paths.applicationsLinkPath);
		fsOps.symlinkSync(paths.canonicalAppPath, paths.applicationsLinkPath, 'dir');
	} catch (error) {
		if (isPermissionDeniedError(error)) {
			return 'skipped';
		}
		throw error;
	}

	return 'updated';
}

function readLinkStats(
	linkPath: string,
	lstat: typeof lstatSync,
): ReturnType<typeof lstatSync> | null | 'permission-denied' {
	try {
		return lstat(linkPath);
	} catch (error) {
		const code = (error as NodeJS.ErrnoException).code;
		if (code === 'ENOENT') {
			return null;
		}
		if (code === 'EACCES' || code === 'EPERM') {
			return 'permission-denied';
		}
		throw error;
	}
}

function isPermissionDeniedError(error: unknown): boolean {
	const code = (error as NodeJS.ErrnoException).code;
	return code === 'EACCES' || code === 'EPERM';
}

function main(): void {
	const repoRoot = path.resolve(import.meta.dir, '..');
	const buildArgs = process.argv.slice(2);

	assertSupportedMacOsHost();
	buildTauriApp(repoRoot, buildArgs);

	if (process.platform !== 'darwin') {
		return;
	}

	const bundlePaths = resolveMacOsBundlePaths(repoRoot);
	verifyMacOsBundle(bundlePaths);

	const linkOutcome = refreshApplicationsLink(bundlePaths);
	if (linkOutcome !== 'skipped') {
		console.log(
			`Refreshed /Applications link: ${bundlePaths.applicationsLinkPath} -> ${bundlePaths.canonicalAppPath}`,
		);
	}
}

if (import.meta.main) {
	main();
}
