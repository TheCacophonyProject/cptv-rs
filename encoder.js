let encoder;
export const createTestCptvFile = async (params) => {
  if (!encoder) {
    encoder = (await import("./encoder/pkg/encoder-node.cjs")).default;
  }
  if (params.recordingDateTime && typeof params.recordingDateTime === 'object' || params.hasOwnProperty('recordingDateTime')) {
    if (!params.recordingDateTime) {
      params.recordingDateTime = new Date(params.recordingDateTime).toISOString();
    } else {
      delete params.recordingDateTime;
    }
  }
  delete params.type;
  delete params.fileHash;
  delete params.processingState;
  delete params.metadata;

  // TODO - add additionalMetadata

  if (params.location) {
    params.longitude = params.location[0];
    params.latitude = params.location[1];
    delete params.location;
  }
  if (params.additionalMetadata) {
    if (params.additionalMetadata.previewSecs) {
      params.previewSecs = params.additionalMetadata.previewSecs;
    }
  }
  const defaultParams = {
    recordingDateTime: new Date().toISOString(),
    deviceName: "Test device",
    deviceId: 99999,
    brand: "Acme",
    model: "lo-res-20",
    serialNumber: 1234,
    firmwareVersion: "1.0",
    latitude: 1,
    duration: 1,
    longitude: 1,
    fps: 1,
    hasBackgroundFrame: false,
  };
  const finalParams = {...defaultParams, ...params};
  console.log('create using params', finalParams);
  return new Uint8Array(encoder.createTestCptvFile(finalParams));
};
