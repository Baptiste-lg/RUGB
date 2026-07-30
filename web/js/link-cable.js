/**
 * Link Cable — BroadcastChannel-based multiplayer for same-device (two tabs).
 *
 * Usage:
 *   const link = new LinkCable(wasmEmu, status => { ... });
 *   link.hostRoom();       // → returns room code
 *   link.joinRoom(code);   // guest joins
 *   link.tick();            // called once per frame in the game loop
 *   link.disconnect();
 */
export class LinkCable {
    constructor(emu, onStatusChange) {
        this.emu = emu;
        this.onStatusChange = onStatusChange || (() => {});
        this.channel = null;
        this.roomCode = null;
        this.connected = false;
        this.isHost = false;
    }

    hostRoom() {
        this.roomCode = this._generateCode();
        this.isHost = true;
        this._openChannel(this.roomCode);
        this.onStatusChange('waiting', this.roomCode);
        return this.roomCode;
    }

    joinRoom(code) {
        this.roomCode = code.toUpperCase().trim();
        this.isHost = false;
        this._openChannel(this.roomCode);
        // Send a join handshake
        this.channel.postMessage({ type: 'join' });
        this.onStatusChange('connecting', this.roomCode);
    }

    disconnect() {
        if (this.channel) {
            this.channel.postMessage({ type: 'disconnect' });
            this.channel.close();
            this.channel = null;
        }
        this.connected = false;
        this.roomCode = null;
        this.onStatusChange('disconnected', null);
    }

    tick() {
        if (!this.emu || !this.connected) return;
        const outgoing = this.emu.serial_take_outgoing();
        if (outgoing !== 0x100 && this.channel) {
            this.channel.postMessage({ type: 'serial', byte: outgoing });
        }
    }

    _openChannel(code) {
        if (this.channel) {
            this.channel.close();
        }
        this.channel = new BroadcastChannel('rugb-link-' + code);
        this.channel.onmessage = (e) => this._onMessage(e.data);
    }

    _onMessage(msg) {
        switch (msg.type) {
            case 'join':
                // Host receives join from guest
                if (this.isHost) {
                    this.connected = true;
                    this.channel.postMessage({ type: 'accept' });
                    this.onStatusChange('connected', this.roomCode);
                }
                break;
            case 'accept':
                // Guest receives accept from host
                if (!this.isHost) {
                    this.connected = true;
                    this.onStatusChange('connected', this.roomCode);
                }
                break;
            case 'serial':
                if (this.emu && this.connected) {
                    this.emu.serial_receive(msg.byte);
                }
                break;
            case 'disconnect':
                this.connected = false;
                this.onStatusChange('peer-disconnected', this.roomCode);
                break;
        }
    }

    _generateCode() {
        const chars = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789'; // no I/O/0/1 to avoid confusion
        const arr = new Uint8Array(6);
        crypto.getRandomValues(arr);
        return Array.from(arr, b => chars[b % chars.length]).join('');
    }
}
