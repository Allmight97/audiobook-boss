import { spawnSync } from 'node:child_process';
import {
	existsSync,
	lstatSync,
	mkdirSync,
	readdirSync,
	readFileSync,
	readlinkSync,
	rmSync,
	symlinkSync,
	unlinkSync,
} from 'node:fs';
import path from 'node:path';

import {
	aaxcleanHelperBaseName,
	publishAaxcleanHelper,
	verifyAaxcleanHelperSidecar,
} from './publish-aaxclean-helper';

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
	helperExecutablePath: string;
	applicationsAppPath: string;
	applicationsLinkPath: string;
	dmgDir: string;
}

type ApplicationsLinkOutcome = 'created' | 'updated' | 'skipped';
type RequestedBundle = 'app' | 'dmg';

interface BuildTauriAppOptions {
	commandRunner?: typeof spawnSync;
	nonInteractiveDmg?: boolean;
}

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

const aaxcleanHelperExternalBin = `binaries/${aaxcleanHelperBaseName}`;

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
		helperExecutablePath: path.join(canonicalAppPath, 'Contents', 'MacOS', aaxcleanHelperBaseName),
		applicationsAppPath: path.join(applicationsDir, `${tauriConfig.productName}.app`),
		applicationsLinkPath: path.join(applicationsDir, `${tauriConfig.productName}.app`),
		dmgDir: path.join(repoRoot, 'target/release/bundle/dmg'),
	};
}

export function resolveRequestedBundles(buildArgs: string[]): Set<RequestedBundle> {
	const bundles = new Set<RequestedBundle>();

	for (let index = 0; index < buildArgs.length; index += 1) {
		const token = buildArgs[index];
		let rawValue: string | null = null;

		if (token === '--bundles' || token === '-b') {
			rawValue = buildArgs[index + 1] ?? '';
			index += 1;
		} else if (token.startsWith('--bundles=')) {
			rawValue = token.slice('--bundles='.length);
		}

		if (rawValue === null) {
			continue;
		}

		for (const entry of rawValue.split(',').map((value) => value.trim().toLowerCase())) {
			if (entry === 'all') {
				bundles.add('app');
				bundles.add('dmg');
				continue;
			}
			if (entry === 'app' || entry === 'dmg') {
				bundles.add(entry);
			}
		}
	}

	if (bundles.size === 0) {
		bundles.add('app');
		bundles.add('dmg');
	}

	return bundles;
}

export function buildTauriApp(
	repoRoot: string,
	buildArgs: string[],
	options: BuildTauriAppOptions = {},
): void {
	publishAaxcleanHelper(repoRoot, options.commandRunner);
	verifyAaxcleanHelperSidecar(repoRoot);

	const tauriArgs = addAaxcleanHelperConfigArg(
		ensureFeatureArg(buildArgs, '--features', 'bundled-ffmpeg'),
	);
	const env = options.nonInteractiveDmg ? { ...process.env, CI: 'true' } : process.env;
	if (options.nonInteractiveDmg) {
		console.log('[build-app] DMG build is noninteractive; Tauri will skip Finder styling.');
	}

	const result = (options.commandRunner ?? spawnSync)(
		'bun',
		['run', 'tauri', 'build', ...tauriArgs],
		{
			cwd: repoRoot,
			env,
			stdio: 'inherit',
		},
	);

	if (typeof result.status === 'number' && result.status !== 0) {
		process.exit(result.status);
	}

	if (result.error) {
		throw result.error;
	}
}

export function addAaxcleanHelperConfigArg(args: string[]): string[] {
	const [tauriArgs, trailingArgs] = splitArgsAtBoundary(args);
	return [
		...tauriArgs,
		'--config',
		JSON.stringify({
			bundle: {
				externalBin: [aaxcleanHelperExternalBin],
			},
		}),
		...trailingArgs,
	];
}

export function verifyMacOsBundle(paths: BundlePaths): void {
	if (!existsSync(paths.canonicalAppPath)) {
		throw new Error(`Expected canonical app bundle at ${paths.canonicalAppPath}`);
	}

	if (!existsSync(paths.executablePath)) {
		throw new Error(`Expected app executable at ${paths.executablePath}`);
	}

	if (!existsSync(paths.helperExecutablePath)) {
		throw new Error(`Expected AAXClean helper executable at ${paths.helperExecutablePath}`);
	}

	verifyMacOsExecutableArchitecture(paths.executablePath);
	verifyMacOsExecutableArchitecture(paths.helperExecutablePath);

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

export function verifyDmgBundle(paths: BundlePaths): void {
	if (!existsSync(paths.dmgDir)) {
		throw new Error(`Expected dmg bundle directory at ${paths.dmgDir}`);
	}

	const dmgArtifacts = readdirSync(paths.dmgDir).filter((entry) => entry.endsWith('.dmg'));
	if (dmgArtifacts.length === 0) {
		throw new Error(`Expected at least one dmg artifact under ${paths.dmgDir}`);
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

export function installLocalApplicationBundle(
	paths: BundlePaths,
	commandRunner: typeof spawnSync = spawnSync,
): void {
	if (!existsSync(paths.canonicalAppPath)) {
		throw new Error(`Expected canonical app bundle at ${paths.canonicalAppPath}`);
	}

	rmSync(paths.applicationsAppPath, { force: true, recursive: true });

	runCheckedCommand(commandRunner, 'ditto', [paths.canonicalAppPath, paths.applicationsAppPath]);
	runCheckedCommand(commandRunner, 'codesign', [
		'--force',
		'--deep',
		'--sign',
		'-',
		paths.applicationsAppPath,
	]);
	runCheckedCommand(commandRunner, 'codesign', [
		'--verify',
		'--deep',
		'--strict',
		'--verbose=2',
		paths.applicationsAppPath,
	]);
	runCheckedCommand(commandRunner, launchServicesRegisterPath(), ['-f', paths.applicationsAppPath]);
	runCheckedCommand(commandRunner, 'mdimport', [paths.applicationsAppPath]);
}

export function pruneLocalInstallArtifacts(paths: BundlePaths): string[] {
	const removedPaths: string[] = [];

	if (existsSync(paths.canonicalAppPath)) {
		rmSync(paths.canonicalAppPath, { force: true, recursive: true });
		removedPaths.push(paths.canonicalAppPath);
	}

	if (existsSync(paths.bundleDir)) {
		for (const entry of readdirSync(paths.bundleDir)) {
			if (!/^rw\.\d+\..+\.dmg$/.test(entry)) {
				continue;
			}
			const artifactPath = path.join(paths.bundleDir, entry);
			rmSync(artifactPath, { force: true, recursive: true });
			removedPaths.push(artifactPath);
		}
	}

	return removedPaths;
}

function runCheckedCommand(commandRunner: typeof spawnSync, command: string, args: string[]): void {
	const result = commandRunner(command, args, { stdio: 'inherit' });
	if (typeof result.status === 'number' && result.status !== 0) {
		throw new Error(`${command} failed with status ${result.status}`);
	}
	if (result.error) {
		throw result.error;
	}
}

function launchServicesRegisterPath(): string {
	return [
		'/System/Library/Frameworks/CoreServices.framework',
		'Frameworks/LaunchServices.framework/Support/lsregister',
	].join('/');
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
	const requestedBundles = resolveRequestedBundles(buildArgs);

	assertSupportedMacOsHost();
	buildTauriApp(repoRoot, buildArgs, { nonInteractiveDmg: requestedBundles.has('dmg') });

	if (process.platform !== 'darwin') {
		return;
	}

	const bundlePaths = resolveMacOsBundlePaths(repoRoot);
	if (requestedBundles.has('app')) {
		verifyMacOsBundle(bundlePaths);
	}

	if (requestedBundles.has('dmg')) {
		verifyDmgBundle(bundlePaths);
	}
}

if (import.meta.main) {
	main();
}
