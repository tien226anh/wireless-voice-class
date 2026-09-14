#!/usr/bin/env node
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { createInterface } from 'node:readline/promises';
import { parseArgs } from 'node:util';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';
import { packageVersion, releaseTags, suggestVersion, validateNewVersion, stampVersion,
  publishRelease } from './release-policy.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');

function command(name, args, options = {}) {
  return execFileSync(name, args, { cwd: root, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024,
    stdio: ['pipe', 'pipe', 'pipe'], ...options }).trim();
}

async function api(method, path, body) {
  const args = ['api', '--method', method, path];
  if (body !== undefined) args.push('--input', '-');
  try {
    const output = command('gh', args, { input: body === undefined ? undefined : JSON.stringify(body) });
    return output ? JSON.parse(output) : null;
  } catch (error) {
    const message = String(error.stderr || error.message);
    const failure = new Error(message);
    failure.status = Number(message.match(/HTTP (\d+)/)?.[1]);
    throw failure;
  }
}

async function prompt(question) {
  if (!process.stdin.isTTY) throw new Error('Use --version (and --yes for manual release) when running noninteractively.');
  const terminal = createInterface({ input: process.stdin, output: process.stdout });
  try { return (await terminal.question(question)).trim(); } finally { terminal.close(); }
}

async function main() {
  const { values, positionals } = parseArgs({
    allowPositionals: true,
    options: {
      version: { type: 'string' }, bump: { type: 'string', default: 'patch' },
      title: { type: 'string' }, 'body-file': { type: 'string' },
      yes: { type: 'boolean', default: false }, help: { type: 'boolean', default: false },
    },
  });
  const operation = positionals[0];
  if (values.help || !operation) {
    console.log(`Usage: node scripts/release.mjs <command> [options]
  suggest [--bump patch|minor|major]     Print the next available release tag
  pr --title TITLE --body-file FILE     Suggest a version, push branch, create labeled PR
     [--version v0.5.1]
  manual [--version v0.5.1] [--yes]      Suggest/confirm a tag, build main, and publish
  stamp --version v0.5.1                CI: stamp Cargo files and VERSION.txt
  publish --version v0.5.1              CI: publish both archives from dist/`);
    return;
  }
  if (!['suggest', 'pr', 'manual', 'stamp', 'publish'].includes(operation)) throw new Error('Unknown command; use --help.');
  if (operation === 'stamp') return stampVersion(values.version, process.env.GITHUB_SHA, root);
  const repo = process.env.GH_REPO || command('gh', ['repo', 'view', '--json', 'nameWithOwner', '--jq', '.nameWithOwner']);
  if (operation === 'publish') {
    const url = await publishRelease({ api, repo, tag: values.version, sha: process.env.GITHUB_SHA,
      directory: resolve(root, 'dist'),
      upload: async (tag, files) => command('gh', ['release', 'upload', tag, ...files, '--clobber', '--repo', repo]),
    });
    console.log(url);
    return;
  }
  const tags = await releaseTags(api, repo);
  const suggestion = suggestVersion(tags, packageVersion(root), values.bump);
  if (operation === 'suggest') return console.log(suggestion);
  if (operation === 'pr') {
    if (!values.title || !values['body-file']) throw new Error('PR creation requires --title and --body-file.');
    readFileSync(resolve(values['body-file']), 'utf8');
    if (command('git', ['status', '--porcelain'])) throw new Error('Commit your changes before creating the PR.');
  }
  console.log(`Suggested release tag: ${suggestion}`);
  const tag = values.version || (await prompt(`Release tag [${suggestion}]: `)) || suggestion;
  validateNewVersion(tag, tags);
  if (operation === 'manual') {
    if (!values.yes && !/^y(es)?$/i.test(await prompt(`Build main and publish ${tag} to ${repo}? [y/N]: `))) {
      console.log('No release started.');
      return;
    }
    command('gh', ['workflow', 'run', 'release.yml', '--repo', repo, '--ref', 'main',
      '-f', 'operation=release', '-f', `version=${tag}`]);
    console.log(`Release requested: https://github.com/${repo}/actions/workflows/release.yml`);
    return;
  }
  const branch = command('git', ['branch', '--show-current']);
  if (!branch || branch === 'main') throw new Error('Create the PR from a feature branch, not main or detached HEAD.');
  command('git', ['push', '-u', 'origin', branch]);
  command('gh', ['label', 'create', `release:${tag}`, '--repo', repo, '--color', '0E8A16',
    '--description', `Publish ${tag} after approval and merge`, '--force']);
  console.log(command('gh', ['pr', 'create', '--repo', repo, '--base', 'main', '--head', branch,
    '--title', values.title, '--body-file', resolve(values['body-file']), '--label', `release:${tag}`]));
}

main().catch(error => {
  console.error(String(error.stderr || error.message));
  process.exitCode = 1;
});
