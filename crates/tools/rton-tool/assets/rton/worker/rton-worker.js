const hardwareConcurrency = Math.max(1, self.navigator?.hardwareConcurrency ?? 1);
const workerCount = Math.min(4, Math.max(1, hardwareConcurrency - 1));
const backend = "worker-pool";

let nextWorkerCursor = 0;
let nextGlobalDocumentId = 1n;

const documentRoutes = new Map();
const reverseDocumentRoutes = new Map();

function errorMessage(error) {
  return error instanceof Error ? error.message : String(error);
}

function collectTransferables(
  value,
  transfer = [],
  seenValues = new Set(),
  seenBuffers = new Set(),
) {
  if (value === null || typeof value !== "object" || seenValues.has(value)) {
    return transfer;
  }
  seenValues.add(value);

  if (ArrayBuffer.isView(value)) {
    const buffer = value.buffer;
    if (buffer instanceof ArrayBuffer && !seenBuffers.has(buffer)) {
      seenBuffers.add(buffer);
      transfer.push(buffer);
    }
    return transfer;
  }
  if (value instanceof ArrayBuffer) {
    if (!seenBuffers.has(value)) {
      seenBuffers.add(value);
      transfer.push(value);
    }
    return transfer;
  }
  if (typeof Blob !== "undefined" && value instanceof Blob) {
    return transfer;
  }

  for (const child of Array.isArray(value) ? value : Object.values(value)) {
    collectTransferables(child, transfer, seenValues, seenBuffers);
  }
  return transfer;
}

class RuntimeWorker {
  constructor(index) {
    this.index = index;
    this.nextRequestId = 1;
    this.pending = new Map();
    this.failed = false;
    this.worker = new Worker(new URL("./rton-worker-runtime.js", import.meta.url), {
      type: "module",
    });
    this.worker.onmessage = (event) => this.handleMessage(event.data);
    this.worker.onerror = (event) => {
      const message = event.message || `RTON runtime worker ${index + 1} failed`;
      this.fail(new Error(message));
    };
  }

  get pendingCount() {
    return this.pending.size;
  }

  handleMessage(message) {
    const pending = this.pending.get(message.id);
    if (!pending) return;
    this.pending.delete(message.id);
    if (message.ok) {
      pending.resolve(message.response);
    } else {
      pending.reject(new Error(message.error || "RTON runtime worker failed"));
    }
  }

  fail(error) {
    this.failed = true;
    for (const pending of this.pending.values()) {
      pending.reject(error);
    }
    this.pending.clear();
  }

  request(message) {
    if (this.failed) {
      return Promise.reject(new Error(`RTON runtime worker ${this.index + 1} is unavailable`));
    }

    const id = this.nextRequestId;
    this.nextRequestId = (this.nextRequestId + 1) || 1;
    const payload = { ...message, id };
    const transfer = collectTransferables(payload);

    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      try {
        this.worker.postMessage(payload, transfer);
      } catch (error) {
        this.pending.delete(id);
        reject(error);
      }
    });
  }
}

const runtimeWorkers = Array.from(
  { length: workerCount },
  (_, index) => new RuntimeWorker(index),
);

function clearWorkerRoutes(workerIndex) {
  for (const route of [...documentRoutes.values()]) {
    if (route.workerIndex === workerIndex) {
      forgetDocumentRoute(route);
    }
  }
}

function runtimeWorker(index) {
  if (runtimeWorkers[index].failed) {
    runtimeWorkers[index].worker.terminate();
    clearWorkerRoutes(index);
    runtimeWorkers[index] = new RuntimeWorker(index);
  }
  return runtimeWorkers[index];
}

function chooseWorkerIndex() {
  let selectedIndex = nextWorkerCursor;
  let selectedPending = Number.POSITIVE_INFINITY;
  for (let offset = 0; offset < workerCount; offset += 1) {
    const index = (nextWorkerCursor + offset) % workerCount;
    const pending = runtimeWorker(index).pendingCount;
    if (pending < selectedPending) {
      selectedIndex = index;
      selectedPending = pending;
    }
  }
  nextWorkerCursor = (selectedIndex + 1) % workerCount;
  return selectedIndex;
}

function documentKey(documentId) {
  return documentId === null || documentId === undefined ? null : String(documentId);
}

function localDocumentKey(workerIndex, documentId) {
  return `${workerIndex}:${documentKey(documentId)}`;
}

function findDocumentRoute(documentId) {
  const key = documentKey(documentId);
  return key === null ? null : documentRoutes.get(key) ?? null;
}

function forgetDocumentRoute(route) {
  documentRoutes.delete(documentKey(route.globalId));
  reverseDocumentRoutes.delete(localDocumentKey(route.workerIndex, route.localId));
}

function exposeDocumentId(response, workerIndex, previousRoute) {
  const localId = response?.worker_document_id;
  if (localId === null || localId === undefined) return;

  if (previousRoute && documentKey(previousRoute.localId) !== documentKey(localId)) {
    forgetDocumentRoute(previousRoute);
  }

  const reverseKey = localDocumentKey(workerIndex, localId);
  let globalId = reverseDocumentRoutes.get(reverseKey);
  if (globalId === undefined) {
    globalId = nextGlobalDocumentId;
    nextGlobalDocumentId += 1n;
    reverseDocumentRoutes.set(reverseKey, globalId);
  }

  documentRoutes.set(documentKey(globalId), { globalId, workerIndex, localId });
  response.worker_document_id = globalId;
}

function hasOwn(value, key) {
  return Object.prototype.hasOwnProperty.call(value, key);
}

function prepareRequest(message) {
  const request = message.request;
  if (!request || typeof request !== "object") {
    return {
      workerIndex: chooseWorkerIndex(),
      previousRoute: null,
      message,
    };
  }

  const localized = { ...request };
  let workerIndex = null;
  let previousRoute = null;

  if (hasOwn(request, "previous_document_id") && request.previous_document_id != null) {
    previousRoute = findDocumentRoute(request.previous_document_id);
    if (previousRoute) {
      workerIndex = previousRoute.workerIndex;
      localized.previous_document_id = previousRoute.localId;
    } else if (request.source != null) {
      localized.previous_document_id = null;
    } else {
      throw new Error("Worker document is no longer available");
    }
  }

  if (hasOwn(request, "document_id") && request.document_id != null) {
    const route = findDocumentRoute(request.document_id);
    if (route) {
      workerIndex = route.workerIndex;
      localized.document_id = route.localId;
    } else if (request.source != null) {
      localized.document_id = null;
    } else {
      throw new Error("Worker document is no longer available");
    }
  }

  const source = request.source;
  if (source && typeof source === "object" && hasOwn(source, "DocumentId")) {
    const route = findDocumentRoute(source.DocumentId);
    if (!route) {
      throw new Error("Worker document is no longer available");
    }
    workerIndex = route.workerIndex;
    localized.source = { DocumentId: route.localId };
  }

  workerIndex ??= chooseWorkerIndex();
  return {
    workerIndex,
    previousRoute,
    message: { ...message, request: localized },
  };
}

async function runBatch(message) {
  const request = message.request;
  const groupedJobs = Array.from({ length: workerCount }, () => []);
  const results = [];

  for (const job of request.jobs ?? []) {
    const source = job.source;
    if (source && typeof source === "object" && hasOwn(source, "DocumentId")) {
      const route = findDocumentRoute(source.DocumentId);
      if (!route) {
        results.push({
          index: job.index,
          bytes: new Uint8Array(),
          error: "Worker document is no longer available",
        });
        continue;
      }
      groupedJobs[route.workerIndex].push({
        ...job,
        source: { DocumentId: route.localId },
      });
      continue;
    }

    groupedJobs[chooseWorkerIndex()].push(job);
  }

  await Promise.all(groupedJobs.map(async (jobs, workerIndex) => {
    if (jobs.length === 0) return;
    const response = await runtimeWorker(workerIndex).request({
      kind: "batch",
      request: { ...request, jobs },
    });
    results.push(...(response.results ?? []));
  }));

  results.sort((left, right) => Number(left.index) - Number(right.index));
  return { results };
}

async function dispatch(message) {
  if (message.kind === "batch") {
    return runBatch(message);
  }

  if (message.kind === "release-document") {
    const route = findDocumentRoute(message.request?.document_id);
    if (!route) return null;
    const response = await runtimeWorker(route.workerIndex).request({
      ...message,
      request: { ...message.request, document_id: route.localId },
    });
    forgetDocumentRoute(route);
    return response;
  }

  const prepared = prepareRequest(message);
  const response = await runtimeWorker(prepared.workerIndex).request(prepared.message);
  exposeDocumentId(response, prepared.workerIndex, prepared.previousRoute);
  return response;
}

console.info(`[RTON Worker] backend=${backend} workers=${workerCount}`);

self.onmessage = async (event) => {
  const id = event.data.id;
  try {
    const response = await dispatch(event.data);
    self.postMessage({
      id,
      ok: true,
      response,
      backend,
      threadCount: workerCount,
      fallbackReason: "",
    }, collectTransferables(response));
  } catch (error) {
    self.postMessage({
      id,
      ok: false,
      backend,
      threadCount: workerCount,
      fallbackReason: "",
      error: errorMessage(error),
    });
  }
};
