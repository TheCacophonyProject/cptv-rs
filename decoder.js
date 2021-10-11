import { Worker as WorkerThread} from "worker_threads";

let messageQueue = {};
let decoder;

export class CptvDecoder {
  constructor() {
    this.free();
    messageQueue = {};
  }

  async init() {
    await this.free();
    messageQueue = {};
    if (!this.inited) {
      const onMessage = (message) => {
        let type;
        let data;
        if (message.type && message.type !== "message") {
          type = message.type;
          data = message.data;
        } else {
          type = message.data.type;
          data = message.data.data;
        }
        const resolver = messageQueue[type];
        delete messageQueue[type];
        resolver && resolver(data);
      };
      if (typeof __ENV__ !== "undefined") {
        // Compiling in a webpack context
        decoder = new Worker(new URL('decoder.worker.js', import.meta.url), {type: "module"});
        decoder.onmessage = onMessage;
      } else {
        // Nodejs usage
        decoder = new WorkerThread(new URL('decoder.worker.js', import.meta.url));
        decoder.addListener.bind(decoder)("message", onMessage);
      }
      await this.waitForMessage("init");
      this.inited = true;
    }
  }

  async initWithCptvUrlAndKnownSize(url, size) {
    await this.init();
    const type = "initWithUrlAndSize";
    decoder.postMessage({ type, url, size });
    return await this.waitForMessage(type);
  }

  async initWithCptvUrl(url) {
    await this.init();
    const type = "initWithUrl";
    decoder.postMessage({ type, url });
    return await this.waitForMessage(type);
  }

  async initWithLocalCptvFile(arrayBuffer) {
    await this.init();
    const type = "initWithLocalCptvFile";
    decoder.postMessage({ type, arrayBuffer });
    return await this.waitForMessage(type);
  }

  async getStreamMetadata(url) {
    await this.init();
    const type = "getStreamMetadata";
    decoder.postMessage({ type, url });
    return await this.waitForMessage(type);
  }

  async getBytesMetadata(arrayBuffer) {
    await this.init();
    const type = "getBytesMetadata";
    decoder.postMessage({ type, arrayBuffer });
    return await this.waitForMessage(type);
  }

  async getNextFrame() {
    const type = "getNextFrame";
    decoder.postMessage({ type });
    return await this.waitForMessage(type);
  }

  async getTotalFrames() {
    const type = "getTotalFrames";
    decoder.postMessage({type});
    return await this.waitForMessage(type);
  }

  async getHeader() {
    const type = "getHeader";
    decoder.postMessage({type});
    return await this.waitForMessage(type);
  }

  async getLoadProgress() {
    const type = "getLoadProgress";
    decoder.postMessage({type});
    return await this.waitForMessage(type);
  }

  async hasStreamError() {
    const type = "hasStreamError";
    decoder.postMessage({type});
    return await this.waitForMessage(type);
  }

  async getStreamError() {
    const type = "getStreamError";
    decoder.postMessage({type});
    return await this.waitForMessage(type);
  }

  async free() {
    const type = "freeResources";
    if (decoder) {
      decoder.postMessage({type});
      return await this.waitForMessage(type);
    }
  }

  async waitForMessage(messageType) {
    if (typeof __ENV__ === "undefined") {
      // In a node context, kill this on idle, but if we're in tests,
      // we save time on initialisation by keeping the same decoder available.
      // Also, no real need to worry about freeing manually.
      clearTimeout(this.killOnIdle);
      this.killOnIdle = setTimeout(async () => {
        await this.close();
      }, 2000);
    }
    return new Promise((resolve) => {
      messageQueue[messageType] = resolve;
    });
  }

  async close() {
    if (typeof __ENV__ === "undefined") {
      clearTimeout(this.killOnIdle);
    }
    if (typeof window === "undefined") {
      decoder && await decoder.terminate();
    } else {
      decoder && decoder.terminate();
    }
  }
}
