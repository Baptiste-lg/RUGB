class RUGBAudioProcessor extends AudioWorkletProcessor {
    constructor() {
        super();
        this._capacity = 16384;
        this._mask = this._capacity - 1;
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
        return (this._writePos - this._readPos + this._capacity) & this._mask;
    }

    _enqueueSamples(left, right) {
        const count = left.length;
        const needed = count * 2;
        const avail = this._available();
        if (avail + needed >= this._capacity) {
            const drop = avail + needed - this._capacity + 2;
            this._readPos = (this._readPos + drop) & this._mask;
        }
        let wp = this._writePos;
        for (let i = 0; i < count; i++) {
            this._buffer[wp] = left[i];
            wp = (wp + 1) & this._mask;
            this._buffer[wp] = right[i];
            wp = (wp + 1) & this._mask;
        }
        this._writePos = wp;
    }

    process(inputs, outputs) {
        const output = outputs[0];
        if (!output || output.length < 2) return true;
        const outL = output[0];
        const outR = output[1];
        const frames = outL.length;
        const available = this._available();
        const stereoFrames = available >> 1;
        const count = Math.min(frames, stereoFrames);

        let rp = this._readPos;
        for (let i = 0; i < count; i++) {
            outL[i] = this._buffer[rp];
            rp = (rp + 1) & this._mask;
            outR[i] = this._buffer[rp];
            rp = (rp + 1) & this._mask;
        }
        this._readPos = rp;

        for (let i = count; i < frames; i++) {
            outL[i] = 0;
            outR[i] = 0;
        }

        if (this._tick++ % 32 === 0) {
            this.port.postMessage({
                type: 'status',
                buffered: this._available() >> 1,
                underrun: count < frames,
            });
        }

        return true;
    }
}

registerProcessor('rugb-audio-processor', RUGBAudioProcessor);
