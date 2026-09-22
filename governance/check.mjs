#!/usr/bin/env node
import { lstat, readFile, readdir } from 'node:fs/promises';
import { basename, dirname, isAbsolute, relative, resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '..');
const fail = (message) => { console.error(`[governance] ${message}`); process.exitCode = 1; };
const sha40 = /^[0-9a-f]{40}$/;
const sha64 = /^[0-9a-f]{64}$/;
const expectedValidatorRevision = 'bd503465dab8c5148fee722b443ed04ff126c9bf';
const forbiddenIngressAuthority = new Set(['handlers.rs', 'funcs.rs', 'resolver.rs', 'lambda.rs', 'rpc.rs']);

function within(base, target) {
  const r = relative(base, target);
  return r === '' || (!r.startsWith(`..${sep}`) && r !== '..' && !isAbsolute(r));
}
function repoPath(value, label) {
  if (typeof value !== 'string' || !value || value.includes('\0') || isAbsolute(value)) throw new Error(`${label} must be repository-relative`);
  const absolute = resolve(root, value);
  if (!within(root, absolute)) throw new Error(`${label} escapes repository root`);
  return absolute;
}
async function realFile(value, label) {
  const absolute = repoPath(value, label);
  const st = await lstat(absolute);
  if (st.isSymbolicLink() || !st.isFile()) throw new Error(`${label} must be a real file: ${value}`);
  return absolute;
}
async function realDirectory(value, label) {
  const absolute = repoPath(value, label);
  const st = await lstat(absolute);
  if (st.isSymbolicLink() || !st.isDirectory()) throw new Error(`${label} must be a real directory: ${value}`);
  return absolute;
}
async function walkFiles(value, label) {
  const start = await realDirectory(value, label);
  const out = [];
  async function walk(directory) {
    for (const entry of (await readdir(directory, { withFileTypes: true })).sort((a, b) => a.name.localeCompare(b.name))) {
      const absolute = resolve(directory, entry.name);
      const st = await lstat(absolute);
      const rel = relative(root, absolute).split(sep).join('/');
      if (st.isSymbolicLink()) throw new Error(`${label} may not traverse symlink: ${rel}`);
      if (st.isDirectory()) await walk(absolute);
      else if (st.isFile()) out.push(rel);
      else throw new Error(`${label} contains unsupported entry: ${rel}`);
    }
  }
  await walk(start);
  return out;
}
function section(text, name) {
  const marker = `[${name}]`;
  const start = text.indexOf(marker);
  if (start < 0) return '';
  const rest = text.slice(start + marker.length);
  const next = rest.search(/^\s*\[[^\]]+\]/m);
  return next < 0 ? rest : rest.slice(0, next);
}

try {
  const policy = JSON.parse(await readFile(await realFile('governance/api-server.v1.json', 'API governance manifest'), 'utf8'));
  if (policy.schema !== 'ores.governance.api-server/v1' || policy.repository !== 'ores-otel/ores-otel-api-server.rs') throw new Error('invalid API governance identity');
  if (policy.packageRole !== 'api' || !sha40.test(policy.apiDocsRevision)) throw new Error('invalid API package/ABI authority');
  if (policy.routeRoot !== 'src/routes' || policy.legacyFlatRoutesAllowed !== false || policy.ingressSemanticAuthorityAllowed !== false) throw new Error('unsafe route-root policy');

  const required = Array.isArray(policy.requiredIngressNamespaces) ? [...policy.requiredIngressNamespaces].sort() : [];
  if (JSON.stringify(required) !== JSON.stringify(['graphql', 'rest', 'rpc', 'ws'])) throw new Error('canonical ingress namespaces must be rest/rpc/graphql/ws');
  const executableRoots = Array.isArray(policy.executableLeafRoots) ? [...policy.executableLeafRoots].sort() : [];
  if (JSON.stringify(executableRoots) !== JSON.stringify(['src/graphql', 'src/routes/rest', 'src/rpc'])) throw new Error('executable leaf roots must be src/routes/rest, src/rpc, src/graphql');
  for (const leafRoot of executableRoots) await realDirectory(leafRoot, `executable leaf root ${leafRoot}`);

  const routeRoot = await realDirectory(policy.routeRoot, 'route root');
  const observedDirs = [];
  const looseFiles = [];
  for (const entry of (await readdir(routeRoot, { withFileTypes: true })).sort((a, b) => a.name.localeCompare(b.name))) {
    if (entry.isSymbolicLink()) throw new Error(`symlink forbidden in route root: ${entry.name}`);
    if (entry.isDirectory()) observedDirs.push(entry.name);
    else if (entry.isFile()) looseFiles.push(entry.name);
    else throw new Error(`unsupported route-root entry: ${entry.name}`);
  }
  if (JSON.stringify(observedDirs.sort()) !== JSON.stringify(required)) throw new Error(`route namespace drift; expected=${required.join(',')} actual=${observedDirs.sort().join(',')}`);
  if (looseFiles.length !== 1 || looseFiles[0] !== 'mod.rs') throw new Error(`legacy flat routes forbidden; route root files=${looseFiles.join(',')}`);
  for (const namespace of required) await realFile(`${policy.routeRoot}/${namespace}/mod.rs`, `${namespace} ingress module`);

  const expectedIngressMounts = {
    rpc: 'src/routes/rpc/v1/route.rs',
    graphql: 'src/routes/graphql/v1/route.rs',
    ws: 'src/routes/ws/v1/route.rs'
  };
  if (!policy.ingressMounts || JSON.stringify(policy.ingressMounts) !== JSON.stringify(expectedIngressMounts)) throw new Error('ingress mount authority drift');
  const expectedIngressPaths = { rpc: '/v1/rpc', graphql: '/v1/graphql', ws: '/ws' };
  for (const namespace of ['rpc', 'graphql', 'ws']) {
    const namespaceRoot = `${policy.routeRoot}/${namespace}`;
    const entries = await readdir(await realDirectory(namespaceRoot, `${namespace} ingress root`), { withFileTypes: true });
    const dirs = entries.filter((entry) => entry.isDirectory()).map((entry) => entry.name).sort();
    const files = entries.filter((entry) => entry.isFile()).map((entry) => entry.name).sort();
    if (JSON.stringify(dirs) !== JSON.stringify(['v1']) || JSON.stringify(files) !== JSON.stringify(['mod.rs'])) throw new Error(`${namespace} ingress root must contain only mod.rs and v1/`);
    await realFile(`${namespaceRoot}/v1/mod.rs`, `${namespace} v1 module`);
    const routePath = policy.ingressMounts[namespace];
    const routeFile = await realFile(routePath, `${namespace} ingress route`);
    const source = await readFile(routeFile, 'utf8');
    if (!source.includes(`pub const PATH: &str = "${expectedIngressPaths[namespace]}";`)) throw new Error(`${namespace} ingress path drift`);
    for (const file of await walkFiles(namespaceRoot, `${namespace} ingress tree`)) {
      if (forbiddenIngressAuthority.has(basename(file))) throw new Error(`semantic authority forbidden under ${namespace} ingress: ${file}`);
    }
  }

  if (!Array.isArray(policy.restLeaves) || policy.restLeaves.length === 0) throw new Error('restLeaves must be non-empty');
  const restRoot = `${policy.routeRoot}/rest`;
  const restEntries = await readdir(await realDirectory(restRoot, 'REST route root'), { withFileTypes: true });
  const restDirs = restEntries.filter((entry) => entry.isDirectory()).map((entry) => entry.name).sort();
  const restFiles = restEntries.filter((entry) => entry.isFile()).map((entry) => entry.name).sort();
  const governedLeaves = [...policy.restLeaves].sort();
  if (JSON.stringify(restDirs) !== JSON.stringify(governedLeaves)) throw new Error(`REST leaf inventory drift; expected=${governedLeaves.join(',')} actual=${restDirs.join(',')}`);
  if (JSON.stringify(restFiles) !== JSON.stringify(['mod.rs'])) throw new Error(`flat REST source forbidden; files=${restFiles.join(',')}`);
  for (const leaf of governedLeaves) {
    if (!/^[A-Za-z0-9_-]+$/.test(leaf)) throw new Error(`invalid governed REST leaf: ${leaf}`);
    await realFile(`${restRoot}/${leaf}/handlers.rs`, `${leaf} handlers authority`);
    await realFile(`${restRoot}/${leaf}/route.rs`, `${leaf} REST projection`);
  }

  const cargo = await readFile(await realFile('Cargo.toml', 'Cargo manifest'), 'utf8');
  const packageMeta = section(cargo, 'package.metadata.ores');
  if (!/^\s*repository_role\s*=\s*"api"\s*$/m.test(packageMeta)) throw new Error('Cargo.toml must declare package.metadata.ores repository_role="api"');
  if (!/^\s*rust-version\s*=\s*"1\.88"\s*$/m.test(section(cargo, 'package'))) throw new Error('Cargo rust-version must remain 1.88');
  for (const dependency of ['ores-api-docs', 'ores-api-docs-operation-macros']) {
    const escaped = dependency.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    const expression = new RegExp(`^\\s*${escaped}\\s*=\\s*\\{[^\\n]*rev\\s*=\\s*"${policy.apiDocsRevision}"[^\\n]*\\}\\s*$`, 'm');
    if (!expression.test(cargo)) throw new Error(`${dependency} must pin api-docs revision ${policy.apiDocsRevision}`);
  }
  const libSource = await readFile(await realFile('src/lib.rs', 'crate root'), 'utf8');
  for (const declaration of ['pub mod graphql;', 'pub mod rpc;', 'pub use state::AppState;']) if (!libSource.includes(declaration)) throw new Error(`crate root missing canonical declaration: ${declaration}`);

  const domain = JSON.parse(await readFile(await realFile(policy.domainAuthorityBinding, 'domain authority binding'), 'utf8'));
  if (domain.schema !== 'ores.contract-authority-binding/v1' || domain.repository !== 'ores-otel/ores-otel-interfaces') throw new Error('invalid OTEL domain authority');
  if (!sha40.test(domain.commit) || domain.path !== 'contracts/' || domain.mode !== 'external-pinned') throw new Error('domain authority must be immutable and externally pinned');
  for (const key of ['contractsSha256', 'corpusSha256', 'conformanceSpecSha256']) if (!sha64.test(domain.receipts?.[key])) throw new Error(`missing/invalid domain authority receipt: ${key}`);

  const admission = JSON.parse(await readFile(await realFile(policy.contractAdmissionManifest, 'contract admission manifest'), 'utf8'));
  if (admission.schema !== 'ores.contract-ir-consumer/v1' || admission.repository !== policy.repository || admission.repositoryRole !== 'api-server') throw new Error('invalid contract admission identity');
  if (admission.canonicalAuthorityRepository !== 'ores-otel/ores-interfaces') throw new Error('local admission canary authority drift');
  if (admission.canonicalAuthorityRepository === domain.repository) throw new Error('domain authority and generic admission canary must remain distinct scopes');
  if (admission.validator?.repository !== 'ORESoftware/typespec-json-schema-validator' || admission.validator?.actionCommit !== expectedValidatorRevision) throw new Error('validator provenance drift');
  if (admission.authorityModel?.precedence !== 'none' || admission.authorityModel?.generatedJsonSchema !== 'comparison-evidence-only' || admission.admission?.allowFallbackAuthority !== false) throw new Error('unsafe peer-authority policy');
  for (const key of ['requirePassedReceipt', 'requireAdmissibleContractIr', 'requireZeroUnexplainedFindings', 'rejectEditableAuthority']) if (admission.admission?.[key] !== true) throw new Error(`contract admission must fail closed: ${key}`);

  await realFile(policy.generatedReadme, 'generated-tree authority marker');
  const conformance = JSON.parse(await readFile(await realFile('conformance/manifest.v1.json', 'conformance manifest'), 'utf8'));
  if (conformance.repository !== policy.repository || conformance.coverage?.status !== 'scaffold-only') throw new Error('API conformance boundary must begin scaffold-only');
  if (conformance.policy?.runtimeSpecificGoldensAllowed !== false || conformance.policy?.generatedEvidenceIsAuthority !== false) throw new Error('unsafe conformance policy');

  console.log(JSON.stringify({
    schema: 'ores.governance.api-server-check/v1',
    repository: policy.repository,
    package_role: policy.packageRole,
    api_docs_revision: policy.apiDocsRevision,
    ingress_namespaces: required,
    ingress_mounts: policy.ingressMounts,
    executable_leaf_roots: executableRoots,
    rest_leaves: governedLeaves,
    domain_authority: { repository: domain.repository, commit: domain.commit },
    admission_canary_authority: admission.canonicalAuthorityRepository,
    validator_revision: admission.validator.actionCommit,
    generated_readme: policy.generatedReadme
  }, null, 2));
} catch (error) {
  fail(error instanceof Error ? error.message : String(error));
}
