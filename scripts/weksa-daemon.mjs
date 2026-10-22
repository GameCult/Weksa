#!/usr/bin/env node
import Module from "node:module";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { mkdir, readdir, rm, writeFile } from "node:fs/promises";
import { basename, delimiter, dirname, extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
configureCultLibModulePath();
const require = Module.createRequire(import.meta.url);
const {
  createIdunnRudpHealthPublisher,
  publishIdunnRudpHealth,
} = require("./idunn-rudp-health.cjs");
const {
  CULTNET_RUDP_PROTOCOL_ID,
  WEKSA_MIMO_VOICEDESIGN_COMMAND_SCHEMA_ID,
  createWeksaRudpCommandServer,
  sendWeksaMimoVoiceDesignCommand,
  formatRudpEndpoint,
} = require("./weksa-rudp-command.cjs");
const options = parseArgs(process.argv.slice(2));
const port = Number(options.port ?? process.env.WEKSA_DAEMON_PORT ?? 8813);
const host = options.host ?? process.env.WEKSA_DAEMON_HOST ?? "127.0.0.1";
const stateRoot = resolve(repoRoot, options.stateRoot ?? process.env.WEKSA_STATE_ROOT ?? ".weksa");
const cultMeshStorePath = resolve(stateRoot, "provider-advertisement-store.cc");
const pidPath = resolve(stateRoot, "weksa-daemon.pid");
const logPath = resolve(stateRoot, "weksa-daemon.log");
const mimoApiKeyPath = options.mimoApiKeyPath ?? process.env.WEKSA_MIMO_API_KEY_FILE ?? "E:/Projects/gamecult-ops/mimo-api-Weksa.txt";
const idunnHealthPublisher = createIdunnRudpHealthPublisher({
  endpoint: options.idunnRudpHealth ?? process.env.WEKSA_IDUNN_RUDP_HEALTH,
  daemonId: options.idunnDaemon ?? process.env.WEKSA_IDUNN_DAEMON ?? "weksa",
  healthContract: options.idunnHealthContract ?? process.env.WEKSA_IDUNN_HEALTH_CONTRACT ?? "weksa.cultnet-rudp-provider-health",
});
const odinCultMeshRudp = options.odinCultMeshRudp ?? process.env.WEKSA_ODIN_CULTMESH_RUDP;
const startedAt = new Date().toISOString();
let tick = 0;
let lastPublishedAt = "";
let lastError = undefined;
let cachedSnapshot = undefined;
let witnessRefreshRunning = false;
let lastRudpHealthSuccessMs = 0;

if (options.health) {
  await runHealthCheck();
  process.exit(0);
}
if (options.mimoCommandJson) {
  const command = JSON.parse(readFileSync(resolve(repoRoot, options.mimoCommandJson), "utf8"));
  const receipt = await sendWeksaMimoVoiceDesignCommand({
    endpoint: formatRudpEndpoint(host, port),
    command,
    runtimeId: "weksa-cli",
    localHost: host.includes(":") ? "::" : "0.0.0.0",
  });
  process.stdout.write(`${JSON.stringify(receipt, null, 2)}\n`);
  process.exit(0);
}

await mkdir(stateRoot, { recursive: true });
await writeFile(pidPath, `${process.pid}\n`, "ascii");
await appendLog(`started pid=${process.pid} host=${host} port=${port}`);
const startupSnapshot = await refreshWitnesses("startup");
await publishOdinStartupRespect(startupSnapshot);
const commandServer = await createWeksaRudpCommandServer({
  host,
  port,
  runtimeId: "weksa-daemon",
  onCommand: async (command) => {
    const receipt = await handleMimoVoiceDesign(command);
    await refreshWitnesses("mimo-voicedesign-command");
    return receipt;
  },
  onError: async (error) => {
    lastError = String(error?.stack ?? error);
    await appendLog(`rudp command error: ${lastError}`);
  },
});
await appendLog(`listening ${commandServer.uri}`);

const interval = setInterval(() => {
  refreshWitnesses("interval").catch(async (error) => {
    lastError = String(error?.stack ?? error);
    await appendLog(`publish error: ${lastError}`);
    await publishWeksaRudpHealth("failed", `Weksa witness refresh failed: ${String(error?.message ?? error)}`);
  });
}, 30_000);

process.on("SIGINT", () => shutdown(0));
process.on("SIGTERM", () => shutdown(0));

async function shutdown(code) {
  clearInterval(interval);
  await appendLog(`stopping pid=${process.pid}`);
  commandServer.close();
  process.exit(code);
}

async function runHealthCheck() {
  const operatorStatePath = resolve(stateRoot, "operator-state.cc");
  const commandBoundaryPath = resolve(stateRoot, "command-boundary.cc");
  const transportProfilePath = resolve(stateRoot, "transport-profile.cc");
  if (!existsSync(pidPath)) {
    throw new Error(`Weksa daemon PID file is missing: ${pidPath}`);
  }
  if (!existsSync(operatorStatePath)) {
    throw new Error(`Weksa operator-state witness is missing: ${operatorStatePath}`);
  }
  if (!existsSync(commandBoundaryPath)) {
    throw new Error(`Weksa command-boundary witness is missing: ${commandBoundaryPath}`);
  }
  if (!existsSync(transportProfilePath)) {
    throw new Error(`Weksa transport-profile witness is missing: ${transportProfilePath}`);
  }
  const pidText = readFileSync(pidPath, "utf8").trim();
  if (!/^\d+$/.test(pidText)) {
    throw new Error(`Weksa daemon PID file is invalid: ${pidText}`);
  }
  const pid = Number(pidText);
  try {
    process.kill(pid, 0);
  } catch {
    throw new Error(`Weksa daemon PID ${pid} is not running.`);
  }
  const operatorStateText = readFileSync(operatorStatePath, "utf8");
  const commandBoundaryText = readFileSync(commandBoundaryPath, "utf8");
  const transportProfileText = readFileSync(transportProfilePath, "utf8");
  const ages = [operatorStatePath, commandBoundaryPath, transportProfilePath]
    .map((path) => Math.floor((Date.now() - statSync(path).mtimeMs) / 1000));
  const ageSeconds = Math.max(...ages);
  if (ageSeconds > 180) {
    throw new Error(`Weksa daemon witnesses are stale (${ageSeconds}s old at oldest required path).`);
  }
  if (!operatorStateText.includes("command_ingress: cultnet_rudp_document_mimo_voicedesign")) {
    throw new Error("Weksa operator-state witness does not report RUDP command ingress.");
  }
  if (!commandBoundaryText.includes(`ingress: ${CULTNET_RUDP_PROTOCOL_ID}`)) {
    throw new Error("Weksa command-boundary witness does not report CultNet/RUDP ingress.");
  }
  if (!commandBoundaryText.includes(`input_schema: ${WEKSA_MIMO_VOICEDESIGN_COMMAND_SCHEMA_ID}`)) {
    throw new Error("Weksa command-boundary witness does not report the MiMo command document schema.");
  }
  if (!transportProfileText.includes(`command_transport: ${CULTNET_RUDP_PROTOCOL_ID}`)) {
    throw new Error("Weksa transport-profile witness does not report CultNet/RUDP command transport.");
  }
  process.stdout.write(`${JSON.stringify({
    ok: true,
    service: "weksa.intent.service",
    state: "healthy",
    pid,
    ageSeconds,
    commandEndpoint: formatRudpEndpoint(host, port),
  }, null, 2)}\n`);
}

async function publishWitnesses() {
  const snapshot = await buildSnapshot();
  await writeWitness("provider-advertisement.cc", snapshot.providerAdvertisement);
  await writeWitness("operator-state.cc", snapshot.operatorState);
  await writeWitness("command-boundary.cc", snapshot.commandBoundary);
  await writeWitness("transport-profile.cc", snapshot.transportProfile);
  await writeWitness("cultmesh-publications.cc", snapshot.cultMeshPublications);
  await writeCultMeshStore(snapshot);
  await writeJson(resolve(stateRoot, "provider-advertisement.json"), snapshot.providerAdvertisement);
  await writeJson(resolve(stateRoot, "operator-state.json"), snapshot.operatorState);
  await writeJson(resolve(stateRoot, "command-boundary.json"), snapshot.commandBoundary);
  await writeJson(resolve(stateRoot, "transport-profile.json"), snapshot.transportProfile);
  await writeJson(resolve(stateRoot, "cultmesh-publications.json"), snapshot.cultMeshPublications);
  cachedSnapshot = snapshot;
  lastPublishedAt = snapshot.operatorState.updated_at;
  tick += 1;
  return snapshot;
}

async function refreshWitnesses(reason) {
  if (witnessRefreshRunning) {
    await appendLog(`skipped overlapping witness refresh reason=${reason}`);
    return cachedSnapshot;
  }
  witnessRefreshRunning = true;
  try {
    const snapshot = await publishWitnesses();
    await publishWeksaRudpHealth(
      "healthy",
      `Weksa refreshed witnesses reason=${reason} tick=${tick} provider=${snapshot.providerAdvertisement.providerId}`,
      snapshot.operatorState.updated_at,
    );
    return snapshot;
  } finally {
    witnessRefreshRunning = false;
  }
}

async function publishWeksaRudpHealth(state, detail, observedAt = new Date().toISOString()) {
  if (!idunnHealthPublisher) return;
  try {
    await publishIdunnRudpHealth(idunnHealthPublisher, { state, detail, observedAt });
    lastRudpHealthSuccessMs = Date.now();
  } catch (error) {
    const hasRecentSuccess = lastRudpHealthSuccessMs > 0 && Date.now() - lastRudpHealthSuccessMs < 300_000;
    if (!hasRecentSuccess) {
      await appendLog(`Idunn RUDP health publish failed: ${String(error?.message ?? error)}`);
    }
  }
}

async function publishOdinStartupRespect(snapshot) {
  if (!odinCultMeshRudp || !snapshot?.providerAdvertisement) return;
  const runtime = loadCultRuntime();
  if (runtime.error) {
    await appendLog(`Odin startup respect skipped: ${runtime.error.message}`);
    return;
  }
  try {
    const { providerAdvertisementDefinition } = defineCultMeshDocuments(runtime.defineDocumentType);
    await runtime.CultMesh.publishRudpDocumentOnce(
      idunnHealthPublisher?.daemonId ?? "weksa",
      0x0d1d0002,
      normalizeRudpEndpoint(odinCultMeshRudp),
      runtime.defineCultNetDocumentBinding({ definition: providerAdvertisementDefinition }),
      snapshot.providerAdvertisement.providerId,
      snapshot.providerAdvertisement,
      {
        sourceRole: "weksa.provider",
        tags: ["startup-respect", "odin-verse-discovery"],
      },
    );
    await appendLog(`Odin startup respect published to ${odinCultMeshRudp}`);
  } catch (error) {
    await appendLog(`Odin startup respect failed: ${String(error?.message ?? error)}`);
  }
}

async function buildSnapshot() {
  const updatedAt = new Date().toISOString();
  const providerAdvertisement = loadProviderAdvertisement(updatedAt);
  const tuning = loadLatestAffectTuning();
  const cultureCatalog = await inspectCultureCatalog();
  const operatorState = {
    schema: "weksa.operator_state.v0",
    service_id: "weksa.intent.service",
    provider_id: providerAdvertisement.providerId,
    verse_id: "asgard.starfire.weksa",
    state: "healthy",
    mode: "daemon",
    pid: process.pid,
    host,
    port,
    started_at: startedAt,
    updated_at: updatedAt,
    last_error: lastError,
    witnesses: {
      provider_advertisement: ".weksa/provider-advertisement.cc",
      provider_advertisement_store: ".weksa/provider-advertisement-store.cc",
      operator_state: ".weksa/operator-state.cc",
      cultmesh_publications: ".weksa/cultmesh-publications.cc",
    },
    readiness: {
      affect_aware_lowering: tuning ? "tuned" : "no_tuning_artifact_seen",
      cultural_ontology_profiles: cultureCatalog.profile_count,
      target_language_profiles: cultureCatalog.target_language_profile_count,
      accepts_commands: true,
      command_ingress: "cultnet_rudp_document_mimo_voicedesign",
      mimo_voicedesign: existsSync(mimoApiKeyPath) ? "api_key_file_seen" : "missing_api_key_file",
    },
    latest_affect_tuning: tuning,
  };
  const commandBoundary = buildCommandBoundary(updatedAt);
  const transportProfile = buildTransportProfile(updatedAt);
  const cultMeshPublications = {
    schema: "weksa.cultmesh_publications.v0",
    service_id: "weksa.intent.service",
    updated_at: updatedAt,
    transport: {
      cultmesh_store: ".weksa/provider-advertisement-store.cc",
      native_cultnet_peer: formatRudpEndpoint(host, port),
      idunn_rudp_health: idunnHealthPublisher ? `${idunnHealthPublisher.endpoint.host}:${idunnHealthPublisher.endpoint.port}` : "unconfigured",
    },
    publications: [
      publication("weksa.service/provider-advertisement", "gamecult.eve.provider_advertisement.v1", ".weksa/provider-advertisement-store.cc", ".weksa/provider-advertisement-store.cc"),
      publication("weksa.operator/status", "weksa.operator_state.v0", ".weksa/operator-state.cc", ".weksa/operator-state.cc"),
      publication("weksa.command-boundary", "weksa.command_boundary.v1", ".weksa/command-boundary.cc", null),
      publication("weksa.transport-profile", "weksa.transport_profile.v1", ".weksa/transport-profile.cc", null),
      publication("weksa.cultmesh/publications", "weksa.cultmesh_publications.v0", ".weksa/cultmesh-publications.cc", ".weksa/cultmesh-publications.cc"),
      publication("weksa.speech-provider/mimo/voicedesign", WEKSA_MIMO_VOICEDESIGN_COMMAND_SCHEMA_ID, ".weksa/speech-provider/mimo/{requestId}/command.cc", formatRudpEndpoint(host, port)),
    ],
  };
  return { providerAdvertisement, operatorState, commandBoundary, transportProfile, cultMeshPublications };
}

async function writeCultMeshStore(snapshot) {
  const runtime = loadCultRuntime();
  if (runtime.error) {
    await appendLog(`CultMesh store write skipped: ${runtime.error.message}`);
    return;
  }
  const {
    providerAdvertisementDefinition,
    operatorStateDefinition,
    commandBoundaryDefinition,
    transportProfileDefinition,
  } = defineCultMeshDocuments(runtime.defineDocumentType);
  const documents = [
    providerAdvertisementDefinition,
    operatorStateDefinition,
    commandBoundaryDefinition,
    transportProfileDefinition,
  ];
  await rm(cultMeshStorePath, { force: true });
  const node = await createCultMeshNodeWithOwnedStoreReset(runtime.CultMesh, cultMeshStorePath, documents);
  await node.put(providerAdvertisementDefinition, snapshot.providerAdvertisement.providerId, snapshot.providerAdvertisement);
  await node.put(operatorStateDefinition, "weksa", snapshot.operatorState);
  await node.put(commandBoundaryDefinition, "weksa", snapshot.commandBoundary);
  await node.put(transportProfileDefinition, "weksa", snapshot.transportProfile);
  await node.flush?.(true);
}

async function createCultMeshNodeWithOwnedStoreReset(CultMesh, storePath, documents) {
  try {
    return await CultMesh.createNode(storePath, { documents });
  } catch (error) {
    await appendLog(`CultMesh store reset after decode failure: ${error.message}`);
    await rm(storePath, { force: true });
    return await CultMesh.createNode(storePath, { documents });
  }
}

function loadCultRuntime() {
  try {
    configureCultLibModulePath();
    const { CultMesh } = require("cultmesh-ts");
    const { defineDocumentType } = require("cultcache-ts");
    const { defineCultNetDocumentBinding } = require("cultnet-ts");
    return { CultMesh, defineDocumentType, defineCultNetDocumentBinding, error: null };
  } catch (error) {
    return { CultMesh: null, defineDocumentType: null, defineCultNetDocumentBinding: null, error };
  }
}

function normalizeRudpEndpoint(endpoint) {
  const text = String(endpoint || "").trim();
  if (!text) {
    throw new Error("Odin CultMesh/RUDP endpoint must be non-empty.");
  }
  return text.toLowerCase().startsWith("rudp://") ? text : `rudp://${text}`;
}

function configureCultLibModulePath() {
  process.env.NODE_PATH = [
    resolve(repoRoot, "..", "CultLib", "packages"),
    process.env.NODE_PATH || "",
  ].filter(Boolean).join(delimiter);
  Module._initPaths();
}

function defineCultMeshDocuments(defineDocumentType) {
  const providerAdvertisementDefinition = defineDocumentType({
    type: "gamecult.eve.provider_advertisement",
    schemaName: "gamecult.eve.provider_advertisement",
    schemaId: "gamecult.eve.provider_advertisement.v1",
    schemaVersion: "gamecult.eve.provider_advertisement.v1",
    global: false,
    name: (value) => value?.providerId || "provider",
    schema: { parse: (value) => value },
  });
  const operatorStateDefinition = defineDocumentType({
    type: "weksa.operator_state",
    schemaName: "weksa.operator_state",
    schemaId: "weksa.operator_state.v0",
    schemaVersion: "weksa.operator_state.v0",
    global: false,
    name: (value) => value?.daemon_id || value?.provider_id || "weksa",
    schema: { parse: (value) => value },
  });
  const commandBoundaryDefinition = defineDocumentType({
    type: "weksa.command_boundary",
    schemaName: "weksa.command_boundary",
    schemaId: "weksa.command_boundary.v1",
    schemaVersion: "weksa.command_boundary.v1",
    global: false,
    name: (value) => value?.boundary_id || value?.daemon_id || "weksa",
    schema: { parse: (value) => value },
  });
  const transportProfileDefinition = defineDocumentType({
    type: "weksa.transport_profile",
    schemaName: "weksa.transport_profile",
    schemaId: "weksa.transport_profile.v1",
    schemaVersion: "weksa.transport_profile.v1",
    global: false,
    name: (value) => value?.profile_id || value?.daemon_id || "weksa",
    schema: { parse: (value) => value },
  });
  return {
    providerAdvertisementDefinition,
    operatorStateDefinition,
    commandBoundaryDefinition,
    transportProfileDefinition,
  };
}

function loadProviderAdvertisement(updatedAt) {
  const fixturePath = resolve(repoRoot, "docs/fixtures/weksa-provider-advertisement.json");
  const fixture = JSON.parse(readFileSync(fixturePath, "utf8"));
  return {
    ...fixture,
    mode: "daemon-live",
    status: "active",
    daemonId: "weksa",
    updatedAt,
    canonicalService: "asgard.weksa",
    locatedService: "asgard.starfire.weksa",
    cultMeshAddress: "asgard.starfire.weksa/operator",
    serviceVerse: {
      ...fixture.serviceVerse,
      semanticAddress: "asgard.starfire.weksa",
      cultMeshKey: "weksa.service/provider-advertisement",
    },
    endpoints: [
      { transport: "cultmesh-store", address: ".weksa/provider-advertisement-store.cc" },
      { transport: CULTNET_RUDP_PROTOCOL_ID, address: formatRudpEndpoint(host, port) },
    ],
    routes: [
      { transport: "cultmesh-store", address: ".weksa/provider-advertisement-store.cc" },
      { transport: CULTNET_RUDP_PROTOCOL_ID, address: formatRudpEndpoint(host, port) },
    ],
    health: {
      state: "healthy",
      endpoint: idunnHealthPublisher ? `${CULTNET_RUDP_PROTOCOL_ID}:idunn-health` : "unconfigured",
      witness: ".weksa/operator-state.cc",
      checkedBy: "idunn.desired_daemon.v1",
    },
  };
}

function buildCommandBoundary(updatedAt) {
  return {
    schema: "weksa.command_boundary.v1",
    boundary_id: "weksa",
    daemon_id: "weksa",
    provider_id: "weksa.intent.service",
    service_id: "weksa.intent.service",
    updated_at: updatedAt,
    owner: "Weksa daemon",
    lifecycle_authority: "idunn.local-command",
    health_publication: {
      contract: "weksa.cultnet-rudp-provider-health",
      transport: CULTNET_RUDP_PROTOCOL_ID,
      publication_source: "daemon-published",
      state_owner: "Weksa daemon",
    },
    commands: [
      {
        command: "speech_provider.mimo.voicedesign",
        ingress: CULTNET_RUDP_PROTOCOL_ID,
        endpoint: formatRudpEndpoint(host, port),
        owner: "Weksa daemon",
        input_schema: WEKSA_MIMO_VOICEDESIGN_COMMAND_SCHEMA_ID,
        output_schema: "weksa.mimo_voicedesign_receipt.v0",
        authority: "Weksa accepts the request, commits typed witnesses, and treats MiMo as an external renderer.",
      },
    ],
    forbidden_writers: [
      "RUDP command senders may request commands but do not own Weksa intent state.",
      "Odin, Eve, and renderers may lower Weksa surfaces but do not mutate command authority.",
    ],
    compatibility: {
      http: null,
      status: "no-http-command-lane",
      cut_line: "The HTTP command route is deleted; Weksa accepts speech_provider.mimo.voicedesign over CultNet/RUDP command documents and keeps typed witnesses as truth.",
    },
  };
}

function buildTransportProfile(updatedAt) {
  return {
    schema: "weksa.transport_profile.v1",
    profile_id: "weksa",
    daemon_id: "weksa",
    provider_id: "weksa.intent.service",
    updated_at: updatedAt,
    target_transport: CULTNET_RUDP_PROTOCOL_ID,
    current_transport: idunnHealthPublisher
      ? "rudp-health-and-cultmesh-store-and-rudp-command"
      : "cultmesh-store-and-rudp-command",
    health_transport: idunnHealthPublisher ? CULTNET_RUDP_PROTOCOL_ID : "unconfigured",
    state_transport: "cultmesh-store",
    command_transport: CULTNET_RUDP_PROTOCOL_ID,
    compatibility: {
      http: null,
      endpoints_are_lowerings: false,
    },
    cut_line: "Weksa health, provider state, and MiMo VoiceDesign command ingress are daemon-owned typed records over CultNet/RUDP plus CultMesh store; HTTP is no longer in the daemon stack.",
  };
}

function loadLatestAffectTuning() {
  const root = resolve(repoRoot, ".weksa-runs/metame-affect-tuning");
  if (!existsSync(root)) {
    return undefined;
  }
  const runs = readdirSyncSafe(root)
    .map((name) => ({ name, path: resolve(root, name, "status.json") }))
    .filter((entry) => existsSync(entry.path))
    .map((entry) => {
      try {
        const status = JSON.parse(readFileSync(entry.path, "utf8"));
        return { entry, status };
      } catch {
        return undefined;
      }
    })
    .filter(Boolean)
    .sort((left, right) => String(right.status.completed_at ?? right.status.started_at ?? "").localeCompare(String(left.status.completed_at ?? left.status.started_at ?? "")));
  const latest = runs[0]?.status;
  if (!latest) {
    return undefined;
  }
  const observations = Array.isArray(latest.observations) ? latest.observations : [];
  const visible = observations.filter((entry) => entry?.evaluation?.verdict === "affect_contrast_visible").length;
  const needsTuning = observations.filter((entry) => entry?.evaluation?.verdict === "needs_tuning").length;
  return {
    run_id: latest.run_id,
    status: latest.status,
    pass_count: latest.pass_count,
    completed_at: latest.completed_at,
    current_prompt_state: latest.current_prompt_state,
    affect_contrast_visible: visible,
    needs_tuning: needsTuning,
  };
}

async function inspectCultureCatalog() {
  const culturalRoot = resolve(repoRoot, "data/cultural-ontology");
  const targetRoot = resolve(repoRoot, "data/target-language-ontology");
  const culturalProfiles = await collectFiles(culturalRoot, ".yaml");
  const targetProfiles = await collectFiles(targetRoot, ".yaml");
  return {
    profile_count: culturalProfiles.length,
    target_language_profile_count: targetProfiles.length,
    cultural_root: "data/cultural-ontology",
    target_language_root: "data/target-language-ontology",
  };
}

async function handleMimoVoiceDesign(body) {
  const command = validateMimoCommand(body);
  const requestId = command.request_id || `mimo-${new Date().toISOString().replace(/[-:.TZ]/g, "")}-${Math.random().toString(36).slice(2, 8)}`;
  const outputRoot = resolve(stateRoot, "speech-provider", "mimo", requestId);
  await mkdir(outputRoot, { recursive: true });
  const commandDocument = {
    ...command,
    schema_version: WEKSA_MIMO_VOICEDESIGN_COMMAND_SCHEMA_ID,
    request_id: requestId,
  };

  const personaEvidence = inspectPersonaState(commandDocument.persona_state_path);
  const interlinguaPacket = commandDocument.interlingua_packet
    ? normalizeInterlinguaPacket(commandDocument.interlingua_packet, requestId)
    : decomposeThoughtIntoInterlingua(commandDocument.thought_text, requestId, personaEvidence, commandDocument);
  const targetRealization = lowerInterlinguaToSpeechText(interlinguaPacket, personaEvidence, commandDocument);
  const mimoRequest = buildMimoVoiceDesignRequest({ requestId, command: commandDocument, personaEvidence, interlinguaPacket, targetRealization });
  const providerResponse = await callMimoVoiceDesign(mimoRequest);
  const audioPath = resolve(outputRoot, `${requestId}.wav`);
  await writeFile(audioPath, providerResponse.audioBytes);

  const receipt = {
    schema_version: "weksa.mimo_voicedesign_receipt.v0",
    request_id: requestId,
    generated_at: new Date().toISOString(),
    ok: true,
    provider: "xiaomi-mimo",
    model: "mimo-v2.5-tts-voicedesign",
    persona_state_ref: personaEvidence.path,
    source_kind: commandDocument.interlingua_packet ? "interlingua_packet" : "thought_text_decomposed_to_interlingua",
    target_language: targetRealization.target_language,
    performance_register: targetRealization.performance_register,
    artifacts: {
      command: `.weksa/speech-provider/mimo/${requestId}/command.cc`,
      interlingua_packet: `.weksa/speech-provider/mimo/${requestId}/interlingua.cc`,
      mimo_request: `.weksa/speech-provider/mimo/${requestId}/mimo-request.cc`,
      receipt: `.weksa/speech-provider/mimo/${requestId}/receipt.cc`,
      audio: `.weksa/speech-provider/mimo/${requestId}/${requestId}.wav`,
    },
    provider_response: {
      id: providerResponse.id,
      created: providerResponse.created,
      audio_bytes: providerResponse.audioBytes.length,
    },
    trace: {
      authority: "Weksa created or accepted interlingua, projected delivery controls, and exported a MiMo VoiceDesign request. MiMo rendered audio only.",
      persona_state_read: personaEvidence.read_status,
      persona_delivery_hints: personaEvidence.delivery_hints,
      text_preview_optimization: false,
      notes: targetRealization.trace.notes,
    },
  };

  await writeWitness(`speech-provider/mimo/${requestId}/command.cc`, commandDocument);
  await writeWitness(`speech-provider/mimo/${requestId}/interlingua.cc`, interlinguaPacket);
  await writeWitness(`speech-provider/mimo/${requestId}/mimo-request.cc`, mimoRequest);
  await writeWitness(`speech-provider/mimo/${requestId}/receipt.cc`, receipt);
  await writeJson(resolve(outputRoot, "command.json"), commandDocument);
  await writeJson(resolve(outputRoot, "interlingua.json"), interlinguaPacket);
  await writeJson(resolve(outputRoot, "mimo-request.json"), mimoRequest);
  await writeJson(resolve(outputRoot, "receipt.json"), receipt);
  await appendLog(`mimo voicedesign request_id=${requestId} audio_bytes=${providerResponse.audioBytes.length}`);
  return receipt;
}

function validateMimoCommand(body) {
  if (!body || typeof body !== "object" || Array.isArray(body)) {
    throw httpError(400, "request body must be a JSON object");
  }
  if (!body.persona_state_path || typeof body.persona_state_path !== "string") {
    throw httpError(400, "persona_state_path is required");
  }
  if (!body.interlingua_packet && !body.thought_text) {
    throw httpError(400, "provide interlingua_packet or thought_text");
  }
  if (body.interlingua_packet && body.thought_text) {
    throw httpError(400, "provide either interlingua_packet or thought_text, not both");
  }
  if (body.thought_text && typeof body.thought_text !== "string") {
    throw httpError(400, "thought_text must be a string");
  }
  return body;
}

function inspectPersonaState(personaStatePath) {
  const absolutePath = resolve(repoRoot, personaStatePath);
  if (!existsSync(absolutePath)) {
    throw httpError(400, `persona_state_path does not exist: ${personaStatePath}`);
  }
  const bytes = readFileSync(absolutePath);
  const printable = Array.from(bytes.subarray(0, Math.min(bytes.length, 4096))).filter((byte) => byte === 9 || byte === 10 || byte === 13 || (byte >= 32 && byte <= 126)).length;
  const printableRatio = bytes.length ? printable / Math.min(bytes.length, 4096) : 0;
  const textPreview = printableRatio > 0.75 ? bytes.toString("utf8", 0, Math.min(bytes.length, 4096)) : "";
  const extractedStrings = extractPersonaStrings(bytes);
  const deliveryHints = derivePersonaDeliveryHints(extractedStrings, basename(absolutePath, extname(absolutePath)));
  return {
    path: absolutePath,
    basename: basename(absolutePath, extname(absolutePath)),
    byte_length: bytes.length,
    read_status: extractedStrings.length ? "string_evidence_extracted" : (printableRatio > 0.75 ? "text_preview_available" : "binary_or_compact_state_reference_only"),
    text_preview: textPreview,
    delivery_hints: deliveryHints,
  };
}

function extractPersonaStrings(bytes) {
  const strings = [];
  let current = "";
  for (const byte of bytes) {
    if (byte >= 32 && byte <= 126) {
      current += String.fromCharCode(byte);
    } else {
      if (current.length >= 8) strings.push(current);
      current = "";
    }
  }
  if (current.length >= 8) strings.push(current);
  return strings
    .filter((text) => /voice|style|presentation|affect|persona|pronoun|she\/her|feminine|woman|female|ship|companion|anime|loli|waifu|sharp|abrasive|technical|bright|cute|hostile|cruel|contempt|protect|dependency|leverage/i.test(text))
    .slice(0, 24);
}

function derivePersonaDeliveryHints(strings, personaId) {
  const joined = strings.join("\n").toLowerCase();
  const hints = [];
  const addIfSeen = (pattern, hint) => {
    if (pattern.test(joined) && !hints.includes(hint)) hints.push(hint);
  };
  addIfSeen(/loli|waifu|companion-shaped|companion product|companion shell/, "youth-coded/loli-coded companion-product presentation as non-sexual product residue");
  addIfSeen(/she\/her|pronoun guidance|feminine|female|woman/, "explicitly feminine she/her voice; do not default masculine");
  addIfSeen(/anime|visual novel/, "anime / visual-novel heroine performance register");
  addIfSeen(/ship mind|ship ai|ship-self|embodied ship|cockpit/, "embodied ship intelligence speaking through ship audio");
  addIfSeen(/abrasive|cutting|bite|cruel|sadistic/, "abrasive, cutting delivery with aim rather than random cruelty");
  addIfSeen(/technical|precision|engineering|mechanism/, "technical precision inside character performance");
  addIfSeen(/bright/, "bright sharp timbre");
  addIfSeen(/dependency|leverage|life support|bargain/, "dependency covered by contempt and leverage");
  addIfSeen(/protect|warning|useful/, "irritated protection rather than soft affection");
  if (!hints.length) hints.push(`${personaId} projected Persona voice`);
  return hints.slice(0, 10);
}

function normalizeInterlinguaPacket(packet, requestId) {
  if (typeof packet === "string") {
    try {
      return { ...JSON.parse(packet), packet_id: requestId };
    } catch {
      return {
        interlingua_version: "0.1-daemon-provisional",
        packet_id: requestId,
        kind: "dialogue_line",
        provenance: { source_type: "opaque_interlingua_text", confidence: 0.45 },
        discourse: { speech_act: "verbalize", intent: packet.slice(0, 240), tone: "unspecified" },
        trace: { notes: ["Interlingua arrived as opaque text; daemon preserved it as intent text rather than parsing YAML in the hot path."] },
      };
    }
  }
  return {
    interlingua_version: packet.interlingua_version ?? "0.1-daemon-provisional",
    packet_id: packet.packet_id ?? requestId,
    kind: packet.kind ?? "dialogue_line",
    ...packet,
  };
}

function decomposeThoughtIntoInterlingua(thoughtText, requestId, personaEvidence, command) {
  const trimmed = thoughtText.trim();
  return {
    interlingua_version: "0.1-daemon-provisional",
    packet_id: requestId,
    kind: "dialogue_line",
    provenance: {
      source_type: "thought_text",
      confidence: 0.55,
      warning: "Heuristic daemon decomposition; review before promoting as canonical interlingua.",
    },
    context: {
      medium: "spoken verbalization",
      scene: command.scene ?? "unspecified",
    },
    discourse: {
      speech_act: command.speech_act ?? inferSpeechAct(trimmed),
      intent: command.intent ?? trimmed,
      tone: command.tone ?? "persona_projected",
      addressee: command.addressee ?? "listener",
    },
    referents: [
      { id: "speaker", kind: "person", label: personaEvidence.basename, salience: "high" },
      { id: "listener", kind: "person", label: command.addressee ?? "listener", salience: "medium" },
    ],
    predications: [
      {
        id: "verbalized-thought",
        predicate: "express",
        frame: "thought_to_speech",
        roles: { agent: "speaker", content: "thought_text", addressee: "listener" },
      },
    ],
    constraints: {
      preserve_text_intent: true,
      required_outputs: ["spoken_text"],
      deterministic: false,
    },
    extensions: {
      "weksa.raw_thought": {
        text: trimmed,
      },
    },
    trace: {
      notes: [
        "Raw thought was converted into provisional interlingua by Weksa daemon command ingress.",
        "This path is for verbalization service use; a reviewer or authoring model should promote durable interlingua if the line becomes canon.",
      ],
    },
  };
}

function inferSpeechAct(text) {
  if (/^(do not|don't|stop|never|take your hand off|hands off|get away from)\b/i.test(text)) return "warning";
  if (/\?$/.test(text)) return "question";
  if (/^(please|could|can you)\b/i.test(text)) return "request";
  return "statement";
}

function lowerInterlinguaToSpeechText(interlinguaPacket, personaEvidence, command) {
  const targetLanguage = {
    code: command.target_language?.code ?? "en",
    locale: command.target_language?.locale ?? "en-US",
    script: command.target_language?.script ?? "Latn",
    register: command.target_language?.register ?? "persona-shaped spoken",
    cultural_ontology_refs: command.target_language?.cultural_ontology_refs ?? ["data/target-language-ontology/en-US.yaml"],
  };
  const performanceRegister = {
    label: command.performance_register?.label ?? "persona verbalization",
    medium: command.performance_register?.medium ?? "spoken character voiceover",
    delivery_archetype: command.performance_register?.delivery_archetype ?? `${personaEvidence.basename} verbalizes projected intent`,
  };
  const spokenText = command.spoken_text
    ?? interlinguaPacket.outputs?.spoken_text
    ?? interlinguaPacket.extensions?.weksa?.spoken_text
    ?? interlinguaPacket.extensions?.["weksa.raw_thought"]?.text
    ?? interlinguaPacket.discourse?.intent
    ?? "";
  if (!spokenText.trim()) {
    throw httpError(400, "could not derive spoken text from command or interlingua packet");
  }
  return {
    realization_id: `${interlinguaPacket.packet_id ?? "packet"}-${targetLanguage.locale}-mimo`,
    target_language: targetLanguage,
    performance_register: performanceRegister,
    outputs: {
      spoken_text: spokenText.trim(),
      private_interpretation: command.private_interpretation ?? "Speaker verbalizes the supplied intent through projected Persona context.",
      intended_effect: command.intended_effect ?? "Make the supplied intent audible in the speaker's voice.",
    },
    trace: {
      notes: [
        "Daemon lowering preserved supplied spoken_text when present; otherwise it used raw thought or interlingua discourse intent.",
        "Target-language register stays separate from provider-facing performance register.",
      ],
    },
  };
}

function buildMimoVoiceDesignRequest({ requestId, command, personaEvidence, interlinguaPacket, targetRealization }) {
  const voiceDesignInstruction = command.voice_design_prompt
    ?? buildVoiceDesignPrompt({ command, personaEvidence, targetRealization });
  const sourceControls = [
    ...personaEvidence.delivery_hints,
    ...(Array.isArray(command.delivery_controls) ? command.delivery_controls : []),
  ].filter((value, index, array) => value && array.indexOf(value) === index);
  return {
    schema_version: "weksa.mimo_tts_request.v0",
    request_id: requestId,
    source_realization_ref: `${interlinguaPacket.packet_id ?? requestId}#${targetRealization.realization_id}`,
    speaker_agent_id: command.speaker_agent_id ?? personaEvidence.basename,
    persona_state_ref: personaEvidence.path,
    provider: {
      id: "xiaomi-mimo",
      model: "mimo-v2.5-tts-voicedesign",
    },
    target_language: targetRealization.target_language,
    performance_register: targetRealization.performance_register,
    voice_design: {
      description: voiceDesignInstruction,
      source_controls: sourceControls,
      projected_context_refs: [personaEvidence.path],
      forbidden_traits: command.forbidden_traits ?? ["generic assistant voice", "provider rewriting source meaning"],
    },
    messages: {
      user_voice_design_instruction: voiceDesignInstruction,
      assistant_spoken_content: command.provider_tags
        ? `${command.provider_tags}${targetRealization.outputs.spoken_text}`
        : targetRealization.outputs.spoken_text,
    },
    audio: {
      format: "wav",
      optimize_text_preview: false,
      streaming: false,
    },
    trace: {
      source_fields: command.interlingua_packet ? ["interlingua_packet"] : ["thought_text", "provisional_interlingua"],
      delivery_controls_used: sourceControls,
      provider_tags_used: command.provider_tags ? [command.provider_tags] : [],
      uncertainties: [
        "VoiceDesign prompt is a lossy provider projection from Weksa-owned delivery intent.",
      ],
    },
  };
}

function buildVoiceDesignPrompt({ command, personaEvidence, targetRealization }) {
  const combinedControls = [
    ...personaEvidence.delivery_hints,
    ...(Array.isArray(command.delivery_controls) ? command.delivery_controls : []),
  ].filter((value, index, array) => value && array.indexOf(value) === index);
  const controls = combinedControls.length
    ? combinedControls.join(", ")
    : "projected Persona voice, clear character delivery, preserve source intent";
  const forbidden = Array.isArray(command.forbidden_traits) && command.forbidden_traits.length
    ? ` Avoid ${command.forbidden_traits.join(", ")}.`
    : " Avoid generic assistant voice and do not rewrite the spoken text.";
  return `${personaEvidence.basename} voice for ${targetRealization.performance_register.label}: ${controls}.${forbidden}`;
}

async function callMimoVoiceDesign(mimoRequest) {
  if (options.mimoDryRun) {
    return {
      id: "weksa-mimo-dry-run",
      created: new Date().toISOString(),
      audioBytes: Buffer.from("RIFF0000WAVEfmt ", "ascii"),
    };
  }
  if (!existsSync(mimoApiKeyPath)) {
    throw httpError(500, `MiMo API key file is missing: ${mimoApiKeyPath}`);
  }
  const apiKey = readFileSync(mimoApiKeyPath, "utf8").trim();
  const providerBody = {
    model: "mimo-v2.5-tts-voicedesign",
    messages: [
      { role: "user", content: mimoRequest.messages.user_voice_design_instruction },
      { role: "assistant", content: mimoRequest.messages.assistant_spoken_content },
    ],
    audio: {
      format: "wav",
      optimize_text_preview: false,
    },
  };
  const providerResponse = await fetch("https://api.xiaomimimo.com/v1/chat/completions", {
    method: "POST",
    headers: {
      "api-key": apiKey,
      "Content-Type": "application/json",
    },
    body: JSON.stringify(providerBody),
  });
  const text = await providerResponse.text();
  if (!providerResponse.ok) {
    throw httpError(502, `MiMo VoiceDesign failed with HTTP ${providerResponse.status}: ${redactProviderError(text)}`);
  }
  const json = JSON.parse(text);
  const audioData = json.choices?.[0]?.message?.audio?.data;
  if (!audioData) {
    throw httpError(502, "MiMo VoiceDesign response did not include audio data");
  }
  return {
    id: json.id ?? null,
    created: json.created ?? null,
    audioBytes: Buffer.from(audioData, "base64"),
  };
}

function publication(key, schema, witness, endpoint) {
  return {
    key,
    schema,
    witness,
    endpoint,
    authority: "Weksa daemon",
  };
}

async function writeWitness(name, value) {
  const path = resolve(stateRoot, name);
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${toCultCacheText(value)}\n`, "utf8");
}

function toCultCacheText(value) {
  return renderYaml(value).trim();
}

function renderYaml(value, indent = 0) {
  const pad = " ".repeat(indent);
  if (Array.isArray(value)) {
    if (!value.length) return "[]";
    return value.map((item) => {
      if (item && typeof item === "object") {
        return `${pad}-\n${renderYaml(item, indent + 2)}`;
      }
      return `${pad}- ${yamlScalar(item)}`;
    }).join("\n");
  }
  if (value && typeof value === "object") {
    return Object.entries(value)
      .filter(([, entryValue]) => entryValue !== undefined)
      .map(([key, entryValue]) => {
        if (entryValue && typeof entryValue === "object") {
          return `${pad}${key}:\n${renderYaml(entryValue, indent + 2)}`;
        }
        return `${pad}${key}: ${yamlScalar(entryValue)}`;
      })
      .join("\n");
  }
  return `${pad}${yamlScalar(value)}`;
}

function yamlScalar(value) {
  if (value === null) return "null";
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  const text = String(value);
  if (!text || /[:#\n\r\t{}[\],&*?|-]|^\s|\s$/.test(text)) {
    return JSON.stringify(text);
  }
  return text;
}

async function collectFiles(root, extension) {
  if (!existsSync(root)) {
    return [];
  }
  const entries = await readdir(root, { withFileTypes: true });
  const results = [];
  for (const entry of entries) {
    const child = resolve(root, entry.name);
    if (entry.isDirectory()) {
      results.push(...await collectFiles(child, extension));
    } else if (entry.name.endsWith(extension)) {
      results.push(child);
    }
  }
  return results;
}

function readdirSyncSafe(path) {
  try {
    return existsSync(path) ? readdirSync(path) : [];
  } catch {
    return [];
  }
}

async function writeJson(path, value) {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function httpError(statusCode, message) {
  const error = new Error(message);
  error.statusCode = statusCode;
  return error;
}

function redactProviderError(text) {
  return String(text).replace(/[A-Za-z0-9_-]{32,}/g, "[redacted]");
}

async function appendLog(line) {
  await mkdir(dirname(logPath), { recursive: true });
  const previous = existsSync(logPath) ? readFileSync(logPath, "utf8") : "";
  await writeFile(logPath, `${previous}[${new Date().toISOString()}] ${line}\n`, "utf8");
}

function parseArgs(args) {
  const parsed = {};
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];
    switch (arg) {
      case "--host":
        parsed.host = args[index + 1];
        index += 1;
        break;
      case "--port":
        parsed.port = args[index + 1];
        index += 1;
        break;
      case "--state-root":
        parsed.stateRoot = args[index + 1];
        index += 1;
        break;
      case "--mimo-api-key-path":
        parsed.mimoApiKeyPath = args[index + 1];
        index += 1;
        break;
      case "--idunn-rudp-health":
        parsed.idunnRudpHealth = args[index + 1];
        index += 1;
        break;
      case "--idunn-daemon":
        parsed.idunnDaemon = args[index + 1];
        index += 1;
        break;
      case "--idunn-health-contract":
        parsed.idunnHealthContract = args[index + 1];
        index += 1;
        break;
      case "--odin-cultmesh-rudp":
        parsed.odinCultMeshRudp = args[index + 1];
        index += 1;
        break;
      case "--health":
        parsed.health = true;
        break;
      case "--mimo-command-json":
        parsed.mimoCommandJson = args[index + 1];
        index += 1;
        break;
      case "--mimo-dry-run":
        parsed.mimoDryRun = true;
        break;
      default:
        throw new Error(`Unknown argument: ${arg}`);
    }
  }
  return parsed;
}
