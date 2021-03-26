import * as cptvPlayer from "./node_modules/cptv_player/cptv_player.js";
export default class CptvPlayer {

  async initWithCptvUrlAndSize(url, size) {
    if (!this.inited) {
      await cptvPlayer.default();
      cptvPlayer.CptvPlayerContext.init();
    } else {
      this.playerContext.free();
      this.reader && this.reader.cancel();
    }
    this.playerContext = cptvPlayer.CptvPlayerContext.new();
    this.response = await fetch(url);
    this.reader = this.response.body.getReader();
    await this.playerContext.initWithReadableStream(this.reader, size);
    this.inited = true;
  }

  getFrameAtIndex(frameNum) {
    const frameData = this.playerContext.getRawFrameN(frameNum);
    if (frameData.length === 0) {
      return false;
    }
    const min = this.playerContext.getMinValue();
    const max = Math.max(this.playerContext.getMaxValue(), min + 300);
    // TODO(jon): Look into adaptive normalisation/histogram schemes
    return { min, max, data: frameData };
  }

  getTotalFrames() {
    if (this.inited && this.playerContext.ptr && this.playerContext.streamComplete()) {
      return this.playerContext.totalFrames();
    }
    return false;
  }

  async seekToFrame(frameNum) {
    if (!this.reader) {
      return "You need to initialise the player with the url of a CPTV file";
    }
    this.playerContext = await cptvPlayer.CptvPlayerContext.seekToFrame(this.playerContext, frameNum);
  }

  async getHeader() {
    if (!this.reader) {
      return "You need to initialise the player with the url of a CPTV file";
    }
    this.playerContext = await cptvPlayer.CptvPlayerContext.fetchHeader(this.playerContext);
    const header = this.playerContext.getHeader();
    this.fps = header.fps;
    return header;
  }

  getFrameHeaderAtIndex(frameNum) {
    return this.playerContext.getFrameHeader(frameNum);
  }

  getCurrentFrame() {
    const frameData = this.playerContext.getRawFrame();
    const min = this.playerContext.getMinValue();
    const max = Math.max(this.playerContext.getMaxValue(), min + 300);
    // TODO(jon): Look into adaptive normalisation/histogram schemes
    return { min, max, data: frameData };
  }

  getBackgroundFrame() {
    return this.playerContext.getBackgroundFrame();
  }

  async abortDownload() {
    return this.reader.cancel();
  }
}
