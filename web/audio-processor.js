class RUGBAudioProcessor extends AudioWorkletProcessor {
    constructor() {
        super();
        this._capacity = 16384;
        this._buffer = new Float32Array(this._capacity);
        this._readPos = 0;
        this._writePos = 0;
        this._tick = 0;

        this.port.onmessage = (e) => {
            if (e.data.type === 'samples') {
                this._enqueueSamples(e.data.left, e.data.right);
            }
        };
    }

    _available() {
        return (this._writePos - this._readPos + this._capacity) % this._capacity;
    }

    _enqueueSamples(left, right) {
        const count = left.length;
        const needed = count * 2;
        const avail = this._available();
        // If buffer would overflow, drop oldest samples
        if (avail + needed >= this._capacity) {
            const drop = avail + needed - this._capacity + 2;
            this._readPos = (this._readPos + drop) % this._capacity;
        }
        for (let i = 0; i < count; i++) {
            this._buffer[this._writePos] = left[i];
            this._writePos = (this._writePos + 1) % this._capacity;
            this._buffer[this._writePos] = right[i];
            this._writePos = (this._writePos + 1) % this._capacity;
        }
    }

    process(inputs, outputs) {
        const output = outputs[0];
        if (!output || output.length < 2) return true;
        const outL = output[0];
        const outR = output[1];
        const frames = outL.length;
        const available = this._available();
        const stereoFrames = Math.floor(available / 2);
        const count = Math.min(frames, stereoFrames);

        for (let i = 0; i < count; i++) {
            outL[i] = this._buffer[this._readPos];
            this._readPos = (this._readPos + 1) % this._capacity;
            outR[i] = this._buffer[this._readPos];
            this._readPos = (this._readPos + 1) % this._capacity;
        }
        for (let i = count; i < frames; i++) {
            outL[i] = 0;
            outR[i] = 0;
        }

        // Report buffer level periodically for adaptive control
        if (this._tick++ % 32 === 0) {
            this.port.postMessage({
                type: 'status',
                buffered: this._available() / 2,
                underrun: count < frames,
            });
        }

        return true;
    }
}

registerProcessor('rugb-audio-processor', RUGBAudioProcessor);
