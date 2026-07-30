/**
 * Link Cable — WebRTC peer-to-peer multiplayer with manual SDP exchange.
 *
 * Flow:
 *   Host clicks "Create Offer" → gets a base64 string to share
 *   Guest pastes the offer, clicks "Accept" → gets an answer string
 *   Host pastes the answer, clicks "Connect" → both are connected
 *
 * Same-device (two tabs) still works via BroadcastChannel fallback.
 */
const MAX_SDP_LENGTH = 65536; // 64 KB cap on SDP strings

export class LinkCable {
    constructor(emu, onStatusChange) {
        this.emu = emu;
        this.onStatusChange = onStatusChange || (() => {});
        this.pc = null;           // RTCPeerConnection
        this.dc = null;           // RTCDataChannel
        this.bcChannel = null;    // BroadcastChannel fallback
        this.connected = false;
        this.isHost = false;
        this.mode = null;         // 'webrtc' or 'local'
        this._sendBuf = new Uint8Array(1); // reusable 1-byte buffer for WebRTC sends
    }

    // ---- Local mode (same device, two tabs) ----

    hostLocal() {
        this.mode = 'local';
        this.isHost = true;
        const code = this._generateCode();
        this.bcChannel = new BroadcastChannel('rugb-link-' + code);
        this.bcChannel.onmessage = (e) => this._onLocalMessage(e.data);
        this.onStatusChange('waiting-local', code);
        return code;
    }

    joinLocal(code) {
        this.mode = 'local';
        this.isHost = false;
        code = code.toUpperCase().trim();
        this.bcChannel = new BroadcastChannel('rugb-link-' + code);
        this.bcChannel.onmessage = (e) => this._onLocalMessage(e.data);
        this.bcChannel.postMessage({ type: 'join' });
        this.onStatusChange('connecting', code);
    }

    _onLocalMessage(msg) {
        switch (msg.type) {
            case 'join':
                if (this.isHost) {
                    this.connected = true;
                    this.bcChannel.postMessage({ type: 'accept' });
                    this.onStatusChange('connected', null);
                }
                break;
            case 'accept':
                if (!this.isHost) {
                    this.connected = true;
                    this.onStatusChange('connected', null);
                }
                break;
            case 'serial':
                if (this.emu && this.connected && typeof msg.byte === 'number') {
                    this.emu.serial_receive(msg.byte & 0xFF);
                }
                break;
            case 'disconnect':
                this.connected = false;
                this.onStatusChange('peer-disconnected', null);
                break;
        }
    }

    // ---- WebRTC mode (cross-device, manual SDP exchange) ----

    async createOffer() {
        this.mode = 'webrtc';
        this.isHost = true;
        this._setupPC();

        this.dc = this.pc.createDataChannel('serial', {
            ordered: true,
            maxRetransmits: 0,
        });
        this._setupDC(this.dc);

        const offer = await this.pc.createOffer();
        await this.pc.setLocalDescription(offer);

        // Wait for ICE gathering to complete
        await this._waitForICE();

        const sdp = btoa(JSON.stringify(this.pc.localDescription));
        this.onStatusChange('offer-ready', sdp);
        return sdp;
    }

    async acceptOffer(offerB64) {
        if (offerB64.length > MAX_SDP_LENGTH) throw new Error('SDP too large');
        this.mode = 'webrtc';
        this.isHost = false;
        this._setupPC();

        this.pc.ondatachannel = (e) => {
            this.dc = e.channel;
            this._setupDC(this.dc);
        };

        const offer = JSON.parse(atob(offerB64));
        if (!offer || !offer.type || !offer.sdp) throw new Error('Invalid offer');
        await this.pc.setRemoteDescription(offer);

        const answer = await this.pc.createAnswer();
        await this.pc.setLocalDescription(answer);

        await this._waitForICE();

        const sdp = btoa(JSON.stringify(this.pc.localDescription));
        this.onStatusChange('answer-ready', sdp);
        return sdp;
    }

    async completeConnection(answerB64) {
        if (answerB64.length > MAX_SDP_LENGTH) throw new Error('SDP too large');
        const answer = JSON.parse(atob(answerB64));
        if (!answer || !answer.type || !answer.sdp) throw new Error('Invalid answer');
        await this.pc.setRemoteDescription(answer);
        this.onStatusChange('connecting', null);
    }

    _setupPC() {
        this.pc = new RTCPeerConnection({
            iceServers: [{ urls: 'stun:stun.l.google.com:19302' }],
        });

        this.pc.oniceconnectionstatechange = () => {
            const state = this.pc.iceConnectionState;
            if (state === 'connected' || state === 'completed') {
                this.connected = true;
                this.onStatusChange('connected', null);
            } else if (state === 'disconnected' || state === 'failed' || state === 'closed') {
                this.connected = false;
                this.onStatusChange('peer-disconnected', null);
            }
        };
    }

    _setupDC(dc) {
        dc.binaryType = 'arraybuffer';
        dc.onopen = () => {
            this.connected = true;
            this.onStatusChange('connected', null);
        };
        dc.onclose = () => {
            this.connected = false;
            this.onStatusChange('peer-disconnected', null);
        };
        dc.onmessage = (e) => {
            if (!this.emu || !this.connected) return;
            if (!(e.data instanceof ArrayBuffer) || e.data.byteLength !== 1) return;
            this.emu.serial_receive(new Uint8Array(e.data)[0]);
        };
    }

    _waitForICE() {
        return new Promise((resolve) => {
            if (this.pc.iceGatheringState === 'complete') {
                resolve();
                return;
            }
            const check = () => {
                if (this.pc.iceGatheringState === 'complete') {
                    this.pc.removeEventListener('icegatheringstatechange', check);
                    resolve();
                }
            };
            this.pc.addEventListener('icegatheringstatechange', check);
            // Timeout fallback — don't wait forever for STUN
            setTimeout(resolve, 5000);
        });
    }

    // ---- Common ----

    tick() {
        if (!this.emu || !this.connected) return;
        const outgoing = this.emu.serial_take_outgoing();
        if (outgoing === 0x100) return;

        if (this.mode === 'local' && this.bcChannel) {
            this.bcChannel.postMessage({ type: 'serial', byte: outgoing });
        } else if (this.mode === 'webrtc' && this.dc && this.dc.readyState === 'open') {
            this._sendBuf[0] = outgoing;
            this.dc.send(this._sendBuf);
        }
    }

    disconnect() {
        if (this.bcChannel) {
            this.bcChannel.postMessage({ type: 'disconnect' });
            this.bcChannel.close();
            this.bcChannel = null;
        }
        if (this.dc) {
            this.dc.close();
            this.dc = null;
        }
        if (this.pc) {
            this.pc.close();
            this.pc = null;
        }
        this.connected = false;
        this.mode = null;
        this.onStatusChange('disconnected', null);
    }

    _generateCode() {
        const chars = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789';
        const arr = new Uint8Array(6);
        crypto.getRandomValues(arr);
        return Array.from(arr, b => chars[b % chars.length]).join('');
    }
}
