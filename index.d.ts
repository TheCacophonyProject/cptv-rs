/* tslint:disable */
/* eslint-disable */
declare class CptvDecoder {
    /**
     * Initialises a new player and associated stream reader.
     * @param url (String)
     * @param size (Number)
     * @returns True on success, or an error string on failure (String | Boolean)
     */
    initWithCptvUrlAndKnownSize(url: string, size: number): Promise<string | boolean>;

    /**
     * Initialises a new player and associated stream reader.
     * @param url (String)
     * @returns True on success, or an error string on failure (String | Boolean)
     */
    initWithCptvUrl(url: string): Promise<string | boolean>;

    /**
     * Initialise a new player with an already loaded local file.
     * @param fileBytes (Uint8Array)
     * @returns True on success, or an error string on failure (String | Boolean)
     */
    initWithLocalCptvFile(fileBytes: Uint8Array): Promise<string | boolean>;

    /**
     * Get the header and duration in seconds for an already loaded byte array
     * This function reads and consumes the entire file, without decoding actual frames.
     * @param fileBytes (Uint8Array)
     */
    getBytesMetadata(fileBytes: Uint8Array): Promise<CptvHeader>;

    /**
     * Get the header and duration of a remote CPTV file given by url.
     * This function reads and consumes the enture file, without decoding actual frames.
     * @param url (String)
     */
    getStreamMetadata(url: string): Promise<CptvHeader>;

    /**
     * If the file stream has completed, this gives the total number
     * of playable frames in the file (excluding any background frame).
     */
    getTotalFrames(): Promise<number | null>;

    /**
     * Get the header for the CPTV file as JSON.
     * Optional fields will always be present, but set to `undefined`
     */
    getHeader(): Promise<CptvHeader>;

    /**
     * Get the next frame in the sequence, if there is one.
     */
    getNextFrame(): Promise<CptvFrame | null>;

    /**
     * Stream load progress from 0..1
     */
    getLoadProgress(): Promise<number>;

    /**
     * Free resources associated with the currently decoded file.
     */
    free(): Promise<void>;

    /**
     * Terminate the decoder worker thread - because the worker thread takes a while to init, ideally we want to
     * do this only when the thread closes.
     */
    close(): Promise<void>;

    /**
     * If the decode halted with errors.  Use this in the API to see if we should continue processing a file, or mark it
     * as damaged.
     */
    hasStreamError(): Promise<boolean>

    /**
     * Get any stream error message
     */
    getStreamError(): Promise<string | null>
}

export interface CptvHeader {
    timestamp: number;
    width: number;
    height: number;
    compression: number;
    deviceName: string;
    fps: number;
    brand: string | null;
    model: string | null;
    deviceId: number | null;
    serialNumber: number | null;
    firmwareVersion: string | null;
    motionConfig: string | null;
    previewSecs: number | null;
    latitude: number | null;
    longitude: number | null;
    locTimestamp: number | null;
    altitude: number | null;
    accuracy: number | null;
    hasBackgroundFrame: boolean;
    // Duration in seconds, *including* any background frame.  This is for compatibility with current
    // durations stored in DB which *include* background frames, the user may wish to subtract 1/fps seconds
    // to get the actual duration.
    // Only set if we used one of the getFileMetadata|getStreamMetadata, and scan the entire file.
    duration?: number;
    totalFrames?: number;

    minValue?: number;
    maxValue?: number;
}

export interface CptvFrameHeader {
    timeOnMs: number;
    lastFfcTimeMs: number | null;
    lastFfcTempC: number | null;
    frameTempC: number | null;
    isBackgroundFrame: boolean;
    imageData: {
        width: number;
        height: number;
        /**
         * Minimum value for this frame
         */
        min: number;
        /**
         * Maximum value for this frame
         */
        max: number;
    }
}

export interface CptvFrame {
    /**
     * Raw u16 data of `width` * `height` length where width and height can be found in the CptvHeader
     */
    data: Uint16Array;

    /**
     * Frame header
     */
    meta: CptvFrameHeader;
}

