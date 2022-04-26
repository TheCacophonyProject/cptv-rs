import {parentPort} from "worker_threads";
let CptvPlayerContext;

async function initWasm() {
  if (!CptvPlayerContext) {
    if (typeof __ENV__ !== "undefined") {
      // Webpack context
      CptvPlayerContext = (await import ("./decoder/pkg/decoder.js")).CptvPlayerContext;
    }
    else {
      CptvPlayerContext = (await import ("./decoder/pkg/decoder-node.cjs")).CptvPlayerContext;
    }
  }
}

class Unlocker {
  constructor() {
    this.fn = null;
  }
  unlock() {
    this.fn && this.fn();
  }
}

// For use in nodejs to wrap an already loaded array buffer into a Reader interface
const FakeReader = function (bytes, maxChunkSize = 0) {
  let state = {
    offsets: [],
    offset: 0
  };
  state.bytes = bytes;
  const length = bytes.byteLength;
  // How many reader chunks to split the file into
  let numParts = 5;
  if (maxChunkSize !== 0) {
    numParts = Math.ceil(length / maxChunkSize);
  }
  const percentages = length / numParts;
  for (let i = 0; i < numParts; i++) {
    state.offsets.push(Math.ceil(percentages * i));
  }
  state.offsets.push(length);
  return {
    read() {
      return new Promise((resolve) => {
        state.offset += 1;
        const value = state.bytes.slice(state.offsets[state.offset - 1], state.offsets[state.offset]);
        resolve({
          value,
          done: state.offset === state.offsets.length - 1
        });
      });
    },
    cancel() {
      // Reset state
      delete state.bytes;
      state = {
        offsets: [],
        offset: 0
      };
      return new Promise((resolve) => {
        resolve()
      });
    }
  }
};

// TODO(jon): This differs depending on whether the sensor is lepton 3 or 3.5
// TODO(jon): This is probably out of scope for this library, should be handled
//  at the player level.
class CptvDecoderInterface {

  free() {
    this.framesRead = 0;
    this.locked = false;
    this.consumed = false;
    this.prevFrameHeader = null;
    this.playerContext && this.playerContext.ptr && this.playerContext.free();
    this.reader && this.reader.cancel();
    delete this.streamError;
    delete this.reader;
    delete this.playerContext;
  }

  hasValidContext() {
    return this.playerContext && this.playerContext.ptr;
  }

  async initWithCptvUrlAndSize(url, size) {
    this.free();
    const unlocker = new Unlocker();
    await this.lockIsUncontended(unlocker);
    this.locked = true;
    try {
      // Use this expired JWT token to test that failure case (usually when a page has been open too long)
      // const oldJWT = "https://api.cacophony.org.nz/api/v1/signedUrl?jwt=eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJfdHlwZSI6ImZpbGVEb3dubG9hZCIsImtleSI6InJhdy8yMDIxLzA0LzE1LzQ3MGU2YjY1LWZkOTgtNDk4Ny1iNWQ3LWQyN2MwOWIxODFhYSIsImZpbGVuYW1lIjoiMjAyMTA0MTUtMTE0MjE2LmNwdHYiLCJtaW1lVHlwZSI6ImFwcGxpY2F0aW9uL3gtY3B0diIsImlhdCI6MTYxODQ2MjUwNiwiZXhwIjoxNjE4NDYzMTA2fQ.p3RAOX7Ns52JqHWTMM5Se-Fn-UCyRtX2tveaGrRmiwo";
      this.consumed = false;
      this.response = await fetch(url);
      if (this.response.status === 200) {
        this.reader = this.response.body.getReader();
        if (!size) {
          size = Number(this.response.headers.get("Content-Length")) || 0;
        }
        this.expectedSize = size;
        await initWasm();
        this.playerContext = await CptvPlayerContext.newWithStream(this.reader);
        unlocker.unlock();
        this.inited = true;
        this.locked = false;
        return true;
      } else {
        unlocker.unlock();
        this.locked = false;
        try {
          const r = await this.response.json();
          return (r.messages && r.messages.pop()) || r.message || "Unknown error";
        } catch (e) {
          return await r.text();
        }
      }
    } catch (e) {
      unlocker.unlock();
      this.locked = false;
      return `Failed to load CPTV url ${url}, ${e}`;
    }
  }

  async initWithFileBytes(fileBytes, filePath = "") {
    this.free();
    this.framesRead = 0;
    this.streamError = undefined;
    const unlocker = new Unlocker();
    await this.lockIsUncontended(unlocker);
    this.locked = true;

    this.reader = new FakeReader(fileBytes, 100000);
    this.expectedSize = fileBytes.length;
    let result;
    try {
      await initWasm();
      this.playerContext = await CptvPlayerContext.newWithStream(this.reader);
      this.inited = true;
      result = true;
    } catch (e) {
      this.streamError = e;
      result = `Failed to load CPTV file ${filePath}, ${e}`;
    }
    unlocker.unlock();
    this.locked = false;
    return result;
  }

  async fetchNextFrame() {
    if (!this.reader) {
      console.warn("You need to initialise the player with the url of a CPTV file");
      return null;
    }
    if (this.consumed) {
      console.warn("Stream has already been consumed and discarded");
      return null;
    }
    const unlocker = new Unlocker();
    await this.lockIsUncontended(unlocker);
    this.locked = true;
    if (this.hasValidContext()) {
      try {
        this.playerContext = await CptvPlayerContext.fetchNextFrame(this.playerContext);
      } catch (e) {
        this.streamError = e;
      }
      if (!this.playerContext || this.playerContext && !this.playerContext.ptr) {
        debugger;
      }
    } else {
      console.warn("Fetch next failed");
    }
    unlocker.unlock();
    this.locked = false;
    if (this.hasStreamError()) {
      return null;
    }
    const frameData = this.playerContext.getNextFrame();
    const frameHeader = this.playerContext.getFrameHeader();
    // NOTE(jon): Work around a bug where the mlx sensor doesn't report timeOn times, just hardcodes 60000
    if (frameHeader && frameHeader.imageData.width !== 32) {
      const sameFrameAsPrev = frameHeader && this.prevFrameHeader && frameHeader.timeOnMs === this.prevFrameHeader.timeOnMs;
      if (sameFrameAsPrev && this.getTotalFrames() === null) {
        debugger;
        const t = this.getTotalFrames();
        this.prevFrameHeader = frameHeader;
        return null;//await this.fetchNextFrame();
      }
      this.prevFrameHeader = frameHeader;
    }
    if (frameData.length === 0) {
      return null;
    }
    this.framesRead++;
    return { data: new Uint16Array(frameData), meta: frameHeader };
  }

  async countTotalFrames() {
    if (!this.reader) {
      console.warn("You need to initialise the player with the url of a CPTV file");
      return 0;
    }
    if (this.hasValidContext()) {
      const unlocker = new Unlocker();
      await this.lockIsUncontended(unlocker);
      this.locked = true;
      try {
        this.playerContext = await CptvPlayerContext.countTotalFrames(this.playerContext);
      } catch (e) {
        this.streamError = e;
      }
      // We can't call any other methods that read frame data on this stream,
      // since we've exhausted it and thrown away the data after scanning for the info we want.
      this.consumed = true;
      unlocker.unlock();
      this.locked = false;
    }
    return this.getTotalFrames();
  }

  async getMetadata() {
    const header = await this.getHeader();
    let totalFrameCount = 0;
    if (this.hasStreamError()) {
      return this.streamError;
    } else {
      if (header.totalFrames) {
        totalFrameCount = header.totalFrames;
      } else {
        totalFrameCount = await this.countTotalFrames();
      }
      const duration = (1 / header.fps) * totalFrameCount;
      return {
        ...header,
        duration,
        totalFrames: totalFrameCount,
      }
    }
  }

  async getBytesMetadata(fileBytes) {
    await this.initWithFileBytes(fileBytes, "", typeof __ENV__ === "undefined");
    return await this.getMetadata();
  }

  async getStreamMetadata(url, size) {
    await this.initWithCptvUrlAndSize(url, size);
    return await this.getMetadata();
  }

  async lockIsUncontended(unlocker) {
    return new Promise((resolve) => {
      if (this.locked) {
        unlocker.fn = resolve;
      } else {
        resolve();
      }
    });
  }

  async getHeader() {
    if (!this.reader) {
      return "You need to initialise the player with the url of a CPTV file";
    }
    if (this.hasValidContext()) {
      const unlocker = new Unlocker();
      await this.lockIsUncontended(unlocker);
      this.locked = true;
      this.playerContext = await CptvPlayerContext.fetchHeader(this.playerContext);
      const header = this.playerContext.getHeader();
      if (header === "Unable to parse header") {
        this.streamError = header;
      }
      unlocker.unlock();
      this.locked = false;
      return header;
    }
    return this.streamError;
  }

  getTotalFrames() {
    if (this.streamError) {
      return this.framesRead;
    }
    if (!this.locked && this.inited && this.hasValidContext() && this.playerContext.streamComplete()) {
      return this.playerContext.totalFrames();
    }
    return null;
  }

  getLoadProgress() {
    if (this.locked || !this.hasValidContext()) {
      return null;
    }
    // This doesn't actually tell us how much has downloaded, just how much has been lazily read.
    return this.playerContext.bytesLoaded() / this.expectedSize;
  }

  hasStreamError() {
    return this.streamError !== undefined;
  }
}

const context = typeof self !== "undefined" ? self : parentPort;
const player = new CptvDecoderInterface();
context.addEventListener("message", async ({data}) => {
  switch (data.type) {
    case "initWithUrl": {
      const result = await player.initWithCptvUrlAndSize(data.url);
      context.postMessage({type: data.type, data: result});
    }
      break;
    case "initWithUrlAndSize": {
      const result = await player.initWithCptvUrlAndSize(data.url, data.size);
      context.postMessage({type: data.type, data: result});
    }
      break;
    case "initWithLocalCptvFile": {
      const result = await player.initWithFileBytes(data.arrayBuffer);
      context.postMessage({type: data.type, data: result});
    }
      break;
    case "getBytesMetadata": {
      const header = await player.getBytesMetadata(data.arrayBuffer);
      context.postMessage({type: data.type, data: header});
    }
      break;
    case "getStreamMetadata": {
      const header = await player.getStreamMetadata(data.url);
      context.postMessage({type: data.type, data: header});
    }
      break;
    case "getNextFrame": {
      const frame = await player.fetchNextFrame();
      context.postMessage({type: data.type, data: frame});
    }
      break;
    case "getTotalFrames": {
      const totalFrames = player.getTotalFrames();
      context.postMessage({type: data.type, data: totalFrames});
    }
      break;
    case "getLoadProgress": {
      const progress = player.getLoadProgress();
      context.postMessage({type: data.type, data: progress});
    }
      break;
    case "getHeader": {
      const header = await player.getHeader();
      context.postMessage({type: data.type, data: header});
    }
      break
    case "hasStreamError": {
      const hasError = player.hasStreamError();
      context.postMessage({type: data.type, data: hasError });
    }
      break;
    case "getStreamError": {
      const error = player.streamError;
      context.postMessage({type: data.type, data: error });
    }
      break;
    case "freeResources": {
      player.free();
      context.postMessage({type: data.type, data: true });
    }
      break;
    default:
      context.postMessage(data);
      return;
  }
});
context.postMessage({type: "init"});

// Needed for webpack
export default {};
