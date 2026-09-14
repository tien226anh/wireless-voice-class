import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, readFileSync, rmSync, statSync, copyFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, basename } from 'node:path';
import { fileURLToPath } from 'node:url';
import { versionParts, suggestVersion, validateNewVersion, prVersion, approvedRevision,
  prepareRelease, publishRelease, stampVersion, packageVersion, list } from './release-policy.mjs';

const sha = 'a'.repeat(40);
const otherSha = 'b'.repeat(40);
const repo = 'owner/wireless-pa';
const pr = {
  number: 3, merged_at: '2026-09-14T10:00:00Z', merge_commit_sha: sha,
  base: { ref: 'main', repo: { full_name: repo } }, head: { sha: otherSha },
  labels: [{ name: 'release:v0.5.0' }],
};
const review = (state, extra = {}) => ({
  state, user: { id: 1 }, submitted_at: '2026-09-14T09:00:00Z', commit_id: otherSha, ...extra,
});

function fixture(overrides = {}) {
  const state = {
    pulls: [structuredClone(pr)], reviews: [review('APPROVED')], tags: [],
    releases: [], permission: 'admin', assets: [], writes: [], ...overrides,
  };
  const api = async (method, path, body) => {
    const url = new URL(path, 'https://api.github.com');
    const route = url.pathname.replace(`/repos/${repo}`, '');
    if (method !== 'GET') state.writes.push({ method, route, body });
    if (method === 'GET') {
      const page = Number(url.searchParams.get('page') || 1);
      const slice = items => items.slice((page - 1) * 100, page * 100);
      if (route === `/commits/${sha}/pulls`) return slice(state.pulls);
      if (route === '/pulls/3/reviews') return slice(state.reviews);
      if (route.endsWith('/permission')) return { permission: state.permission };
      if (route === '/tags') return slice(state.tags);
      if (route === '/releases') return slice(state.releases);
      if (route === '/releases/1/assets') return slice(state.assets);
      if (route.startsWith('/git/ref/tags/')) {
        const tag = state.tags.find(t => t.name === route.split('/').at(-1));
        if (tag) return { object: { type: 'commit', sha: tag.sha || sha } };
      }
    }
    if (method === 'POST' && route === '/releases/generate-notes') return { body: 'Release notes' };
    if (method === 'POST' && route === '/releases') {
      const release = { ...body, id: 1, html_url: `https://github.com/${repo}/releases/tag/${body.tag_name}` };
      state.releases.push(release);
      return release;
    }
    if (method === 'POST' && route === '/git/refs') {
      state.tags.push({ name: body.ref.split('/').at(-1), sha: body.sha });
      return {};
    }
    if (method === 'PATCH' && route === '/releases/1') {
      Object.assign(state.releases[0], body);
      return state.releases[0];
    }
    throw Object.assign(new Error(`Not found: ${method} ${route}`), { status: 404 });
  };
  return { state, api };
}

function plan(api, extra = {}) {
  return prepareRelease({ api, repo, eventName: 'push', event: {}, sha,
    ref: 'refs/heads/main', actor: 'owner', baseline: 'v0.5.0', ...extra });
}

function published(draft = false, commit = sha) {
  return { id: 1, tag_name: 'v0.5.0', target_commitish: commit, draft,
    body: `<!-- wireless-pa-release:${commit} -->`, html_url: 'https://example.com/release' };
}

test('suggests the manifest version first, then compares semantic versions numerically', () => {
  assert.equal(suggestVersion([], 'v0.5.0'), 'v0.5.0');
  assert.equal(suggestVersion(['v0.9.0', 'v0.10.0'], 'v0.5.0'), 'v0.10.1');
  assert.equal(suggestVersion(['v0.5.0-pr.1'], 'v0.5.0'), 'v0.5.0');
  assert.equal(suggestVersion(['v0.5.0'], 'v0.6.0'), 'v0.6.0');
  assert.equal(suggestVersion(['v0.5.9'], 'v0.5.0', 'minor'), 'v0.6.0');
  assert.equal(suggestVersion(['v0.5.9'], 'v0.5.0', 'major'), 'v1.0.0');
});

test('rejects ambiguous, unsafe, duplicate, and older versions', () => {
  for (const tag of ['0.5.1', 'v01.2.3', 'v1.2', 'v1.2.3-beta', 'v1.2.3\n', 'v1.2.3;echo x', 'v9007199254740992.0.0']) {
    assert.throws(() => versionParts(tag));
  }
  assert.throws(() => validateNewVersion('v0.5.0', ['v0.5.0']), /already exists/);
  assert.throws(() => validateNewVersion('v0.5.0', ['v0.6.0']), /newer/);
  assert.equal(validateNewVersion('v0.5.1', ['v0.5.0']), 'v0.5.1');
});

test('requires exactly one exact-version release label', () => {
  assert.equal(prVersion([{ name: 'docs' }]), null);
  assert.equal(prVersion(pr.labels), 'v0.5.0');
  assert.throws(() => prVersion([{ name: 'release:patch' }]));
  assert.throws(() => prVersion([...pr.labels, { name: 'release:v0.6.0' }]), /exactly one/);
});

const reviewCases = [
  ['no reviews', [], false],
  ['current approval', [review('APPROVED')], true],
  ['stale approval', [review('APPROVED', { commit_id: sha })], false],
  ['dismissed approval', [review('DISMISSED')], false],
  ['changes requested after approval', [review('APPROVED'), review('CHANGES_REQUESTED')], false],
  ['approval after changes requested', [review('CHANGES_REQUESTED'), review('APPROVED')], true],
  ['comment does not revoke approval', [review('APPROVED'), review('COMMENTED')], true],
  ['another reviewer blocks', [review('APPROVED'), review('CHANGES_REQUESTED', { user: { id: 2 } })], false],
  ['post-merge approval is not accepted', [review('APPROVED', { submitted_at: '2026-09-14T11:00:00Z' })], false],
];
for (const [name, reviews, expected] of reviewCases) {
  test(`review policy: ${name}`, () => assert.equal(approvedRevision(pr, reviews), expected));
}

test('approved tagged merge plans the exact version and source commit without writes', async () => {
  const { api, state } = fixture();
  const result = await plan(api);
  assert.equal(result.run, true);
  assert.equal(result.tag, 'v0.5.0');
  assert.equal(result.sha, sha);
  assert.equal(result.prNumber, '3');
  assert.deepEqual(state.writes, []);
});

for (const [name, overrides] of [
  ['direct push', { pulls: [] }],
  ['unapproved merge', { reviews: [] }],
  ['untagged merge', { pulls: [{ ...pr, labels: [] }] }],
  ['unmerged PR', { pulls: [{ ...pr, merged_at: null }] }],
  ['another merge commit', { pulls: [{ ...pr, merge_commit_sha: otherSha }] }],
]) {
  test(`does not build on ${name}`, async () => assert.equal((await plan(fixture(overrides).api)).run, false));
}

test('manual suggestion is read-only and accounts for reserved draft versions', async () => {
  const { api, state } = fixture({ releases: [published(true)] });
  const result = await plan(api, { eventName: 'workflow_dispatch', event: { inputs: { operation: 'suggest', bump: 'patch' } } });
  assert.equal(result.run, false);
  assert.equal(result.tag, 'v0.5.1');
  assert.deepEqual(state.writes, []);
});

test('manual release allows the owner without a PR review', async () => {
  const { api } = fixture({ pulls: [], reviews: [] });
  const result = await plan(api, { eventName: 'workflow_dispatch', event: { inputs: { operation: 'release', version: 'v0.5.0' } } });
  assert.equal(result.run, true);
});

test('manual release requires main, write access, and an explicit valid version', async () => {
  const manual = { eventName: 'workflow_dispatch', event: { inputs: { operation: 'release' } } };
  await assert.rejects(plan(fixture().api, { ...manual, ref: 'refs/heads/feature' }), /main/);
  await assert.rejects(plan(fixture({ permission: 'read' }).api, manual), /access/);
  await assert.rejects(plan(fixture().api, manual), /explicitly/);
  await assert.rejects(plan(fixture().api, { eventName: 'pull_request' }), /Unsupported/);
});

test('retries recognize only releases owned by this exact source commit', async () => {
  assert.equal((await plan(fixture({ releases: [published(true)] }).api)).run, true);
  assert.equal((await plan(fixture({ releases: [published()], tags: [{ name: 'v0.5.0', sha }] }).api)).run, false);
  await assert.rejects(plan(fixture({ releases: [published(true, otherSha)] }).api), /another release/);
  await assert.rejects(plan(fixture({ tags: [{ name: 'v0.5.0', sha }] }).api), /already exists/);
  await assert.rejects(plan(fixture({ releases: [{ ...published(true), body: 'External release' }] }).api), /another release/);
});

test('API errors fail closed rather than treating the version as available', async () => {
  const { api } = fixture();
  await assert.rejects(plan(async (method, path, data) => {
    if (path.includes('/releases?')) throw Object.assign(new Error('Denied'), { status: 403 });
    return api(method, path, data);
  }), /Denied/);
});

test('list fetches all pages', async () => {
  const calls = [];
  const result = await list(async (_method, path) => {
    calls.push(path);
    return path.endsWith('page=1') ? Array(100).fill({}) : [{ last: true }];
  }, '/items');
  assert.equal(result.length, 101);
  assert.equal(calls.length, 2);
});

test('version stamping changes only the root package and records the source SHA', t => {
  const root = mkdtempSync(join(tmpdir(), 'wireless-version-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  copyFileSync(new URL('../Cargo.toml', import.meta.url), join(root, 'Cargo.toml'));
  copyFileSync(new URL('../Cargo.lock', import.meta.url), join(root, 'Cargo.lock'));
  const before = readFileSync(join(root, 'Cargo.lock'), 'utf8');
  stampVersion('v0.6.1', sha, root);
  assert.equal(packageVersion(root), 'v0.6.1');
  assert.equal(readFileSync(join(root, 'Cargo.lock'), 'utf8'), before.replace('name = "wireless-pa"\nversion = "0.5.0"', 'name = "wireless-pa"\nversion = "0.6.1"'));
  assert.match(readFileSync(join(root, 'VERSION.txt'), 'utf8'), new RegExp(sha));
});

function publication(t, overrides = {}) {
  const directory = mkdtempSync(join(tmpdir(), 'wireless-publish-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  for (const name of ['wireless-pa-v0.5.0-linux-x64.tar.gz', 'wireless-pa-v0.5.0-windows-x64.zip']) {
    writeFileSync(join(directory, name), 'Test archive bytes');
  }
  const { state, api } = fixture(overrides);
  const upload = async (_tag, paths) => {
    state.assets = paths.map(path => ({ name: basename(path), state: 'uploaded', size: statSync(path).size }));
  };
  return { state, options: { api, upload, repo, tag: 'v0.5.0', sha, directory } };
}

test('publication uploads both archives and checksums before pinning tag and publishing Latest', async t => {
  const { state, options } = publication(t);
  const url = await publishRelease(options);
  assert.match(url, /releases\/tag\/v0.5.0$/);
  assert.equal(state.assets.length, 3);
  assert.equal(state.tags[0].sha, sha);
  assert.equal(state.releases[0].draft, false);
  assert.equal(state.releases[0].make_latest, 'true');
  assert.match(readFileSync(join(options.directory, 'SHA256SUMS'), 'utf8'), /^[a-f0-9]{64}  wireless-pa-v0.5.0-linux-x64.tar.gz\n/);
  assert.deepEqual(state.writes.map(w => w.route), ['/releases/generate-notes', '/releases', '/git/refs', '/releases/1']);
});

test('failed upload leaves a draft without a tag; retry completes the same release', async t => {
  const { state, options } = publication(t);
  await assert.rejects(publishRelease({ ...options, upload: async () => { throw new Error('Upload failed'); } }), /Upload failed/);
  assert.equal(state.releases.length, 1);
  assert.equal(state.releases[0].draft, true);
  assert.equal(state.tags.length, 0);
  await publishRelease(options);
  assert.equal(state.releases.length, 1);
  assert.equal(state.releases[0].draft, false);
});

test('missing or incomplete assets cannot publish', async t => {
  const { state, options } = publication(t);
  await assert.rejects(publishRelease({ ...options, upload: async () => {} }), /not uploaded completely/);
  assert.equal(state.releases[0].draft, true);
  assert.equal(state.tags.length, 0);
  rmSync(join(options.directory, 'wireless-pa-v0.5.0-windows-x64.zip'));
  state.writes = [];
  await assert.rejects(publishRelease(options), /ENOENT/);
  assert.deepEqual(state.writes, []);
});

test('a tag moved by another actor during upload cannot be published', async t => {
  const { state, options } = publication(t);
  await assert.rejects(publishRelease({ ...options, upload: async (...args) => {
    await options.upload(...args);
    state.tags.push({ name: 'v0.5.0', sha: otherSha });
  } }), /another commit/);
  assert.equal(state.releases[0].draft, true);
});

test('a completed release is not uploaded or edited again', async t => {
  const { state, options } = publication(t, { releases: [published()], tags: [{ name: 'v0.5.0', sha }] });
  await publishRelease({ ...options, upload: async () => assert.fail('Must not replace published assets') });
  assert.deepEqual(state.writes, []);
});

test('manual CLI dispatches the confirmed version on main and requires confirmation', t => {
  const directory = mkdtempSync(join(tmpdir(), 'wireless-cli-'));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const log = join(directory, 'calls.jsonl');
  writeFileSync(join(directory, 'gh'), `#!/usr/bin/env node
const fs = require('node:fs');
const args = process.argv.slice(2);
fs.appendFileSync(process.env.RELEASE_TEST_LOG, JSON.stringify(args) + '\\n');
if (args[0] === 'api') console.log('[]');
`, { mode: 0o755 });
  const cli = fileURLToPath(new URL('./release.mjs', import.meta.url));
  const options = { encoding: 'utf8', env: { ...process.env, GH_REPO: repo,
    PATH: `${directory}:${process.env.PATH}`, RELEASE_TEST_LOG: log } };
  const confirmed = spawnSync(process.execPath, [cli, 'manual', '--version', 'v0.6.0', '--yes'], options);
  assert.equal(confirmed.status, 0, JSON.stringify(confirmed));
  assert.match(confirmed.stdout, /Suggested release tag: v0.5.0/);
  let calls = readFileSync(log, 'utf8').trim().split('\n').map(JSON.parse);
  assert.deepEqual(calls.find(args => args[0] === 'workflow'),
    ['workflow', 'run', 'release.yml', '--repo', repo, '--ref', 'main', '-f', 'operation=release', '-f', 'version=v0.6.0']);
  writeFileSync(log, '');
  const unconfirmed = spawnSync(process.execPath, [cli, 'manual', '--version', 'v0.6.0'], options);
  assert.equal(unconfirmed.status, 1);
  calls = readFileSync(log, 'utf8').trim().split('\n').map(JSON.parse);
  assert.equal(calls.some(args => args[0] === 'workflow'), false);
});
