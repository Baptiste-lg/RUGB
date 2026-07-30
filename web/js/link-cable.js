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
                if (this.emu && this.connected) {
                    this.emu.serial_receive(msg.byte);
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
        this.mode = 'webrtc';
        this.isHost = false;
        this._setupPC();

        this.pc.ondatachannel = (e) => {
            this.dc = e.channel;
            this._setupDC(this.dc);
        };

        const offer = JSON.parse(atob(offerB64));
        await this.pc.setRemoteDescription(offer);

        const answer = await this.pc.createAnswer();
        await this.pc.setLocalDescription(answer);

        await this._waitForICE();

        const sdp = btoa(JSON.stringify(this.pc.localDescription));
        this.onStatusChange('answer-ready', sdp);
        return sdp;
    }

    async completeConnection(answerB64) {
        const answer = JSON.parse(atob(answerB64));
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
            if (this.emu && this.connected) {
                const byte = new Uint8Array(e.data)[0];
                this.emu.serial_receive(byte);
            }
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
            this.dc.send(new Uint8Array([outgoing]));
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
