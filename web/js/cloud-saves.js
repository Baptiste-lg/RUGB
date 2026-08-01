/**
 * Cloud Saves for RUGB — Google Drive API integration.
 * Uses the "appDataFolder" (hidden, app-only) so user's Drive stays clean.
 * OAuth 2.0 implicit grant — no backend required.
 */

// Replace with your own OAuth client ID from Google Cloud Console
const CLIENT_ID = '';
const SCOPES = 'https://www.googleapis.com/auth/drive.appdata';
const DISCOVERY_DOC = 'https://www.googleapis.com/discovery/v1/apis/drive/v3/rest';

export class CloudSaves {
    constructor() {
        this.tokenClient = null;
        this.accessToken = null;
        this.gapiLoaded = false;
        this.gisLoaded = false;
        this._onSignIn = null;
    }

    /* ── Library loading ──────────────────────────────────────────── */

    async init() {
        await Promise.all([this._loadGapi(), this._loadGis()]);
    }

    _loadGapi() {
        if (this.gapiLoaded) return Promise.resolve();
        return new Promise((resolve, reject) => {
            if (window.gapi) { this._initGapi().then(resolve); return; }
            const s = document.createElement('script');
            s.src = 'https://apis.google.com/js/api.js';
            s.onload = () => this._initGapi().then(resolve);
            s.onerror = reject;
            document.head.appendChild(s);
        });
    }

    async _initGapi() {
        await new Promise(r => gapi.load('client', r));
        await gapi.client.init({ discoveryDocs: [DISCOVERY_DOC] });
        this.gapiLoaded = true;
        // Restore token from sessionStorage
        const saved = sessionStorage.getItem('rugb-cloud-token');
        if (saved) {
            this.accessToken = saved;
            gapi.client.setToken({ access_token: saved });
        }
    }

    _loadGis() {
        if (this.gisLoaded) return Promise.resolve();
        return new Promise((resolve, reject) => {
            if (window.google && window.google.accounts) { this._initGis(); resolve(); return; }
            const s = document.createElement('script');
            s.src = 'https://accounts.google.com/gsi/client';
            s.onload = () => { this._initGis(); resolve(); };
            s.onerror = reject;
            document.head.appendChild(s);
        });
    }

    _initGis() {
        this.tokenClient = google.accounts.oauth2.initTokenClient({
            client_id: CLIENT_ID,
            scope: SCOPES,
            callback: (resp) => {
                if (resp.error) return;
                this.accessToken = resp.access_token;
                sessionStorage.setItem('rugb-cloud-token', resp.access_token);
                gapi.client.setToken({ access_token: resp.access_token });
                if (this._onSignIn) this._onSignIn();
            },
        });
        this.gisLoaded = true;
    }

    /* ── Auth ──────────────────────────────────────────────────────── */

    signIn() {
        return new Promise((resolve) => {
            this._onSignIn = resolve;
            if (this.accessToken) {
                this.tokenClient.requestAccessToken({ prompt: '' });
            } else {
                this.tokenClient.requestAccessToken({ prompt: 'consent' });
            }
        });
    }

    signOut() {
        if (this.accessToken) {
            google.accounts.oauth2.revoke(this.accessToken);
        }
        this.accessToken = null;
        sessionStorage.removeItem('rugb-cloud-token');
        gapi.client.setToken(null);
    }

    isSignedIn() {
        return !!this.accessToken;
    }

    /* ── Drive operations ─────────────────────────────────────────── */

    async listSaves() {
        const resp = await gapi.client.drive.files.list({
            spaces: 'appDataFolder',
            fields: 'files(id, name, modifiedTime, appProperties)',
            pageSize: 100,
        });
        return resp.result.files || [];
    }

    async uploadSave(name, data, metadata = {}) {
        const file = new Blob([data], { type: 'application/octet-stream' });
        const meta = {
            name,
            parents: ['appDataFolder'],
            appProperties: {
                gameTitle: metadata.title || '',
                timestamp: metadata.timestamp || new Date().toISOString(),
                system: metadata.system || '',
            },
        };

        // Check if file with same name exists — update instead of create
        const existing = await this.listSaves();
        const match = existing.find(f => f.name === name);

        const form = new FormData();
        form.append('metadata', new Blob([JSON.stringify(meta)], { type: 'application/json' }));
        form.append('file', file);

        const url = match
            ? `https://www.googleapis.com/upload/drive/v3/files/${match.id}?uploadType=multipart`
            : 'https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart';
        const method = match ? 'PATCH' : 'POST';

        const resp = await fetch(url, {
            method,
            headers: { Authorization: `Bearer ${this.accessToken}` },
            body: form,
        });

        if (!resp.ok) throw new Error(`Upload failed: ${resp.status}`);
        return resp.json();
    }

    async downloadSave(fileId) {
        const resp = await fetch(
            `https://www.googleapis.com/drive/v3/files/${fileId}?alt=media`,
            { headers: { Authorization: `Bearer ${this.accessToken}` } }
        );
        if (!resp.ok) throw new Error(`Download failed: ${resp.status}`);
        return new Uint8Array(await resp.arrayBuffer());
    }

    async deleteSave(fileId) {
        await gapi.client.drive.files.delete({ fileId });
    }

    /* ── Sync logic ───────────────────────────────────────────────── */

    async syncAll() {
        if (!this.isSignedIn()) return { uploaded: 0, downloaded: 0 };

        const cloudFiles = await this.listSaves();
        const cloudMap = new Map();
        for (const f of cloudFiles) cloudMap.set(f.name, f);

        let uploaded = 0, downloaded = 0;

        // Sync save slots (1-5)
        for (let i = 1; i <= 5; i++) {
            const key = `rugb-slot-${i}`;
            const local = localStorage.getItem(key);
            const cloudName = `slot-${i}.sav`;
            const cloud = cloudMap.get(cloudName);

            if (local && !cloud) {
                // Local only — upload
                const parsed = JSON.parse(local);
                const data = Uint8Array.from(atob(parsed.data), c => c.charCodeAt(0));
                await this.uploadSave(cloudName, data, {
                    title: parsed.title,
                    timestamp: parsed.timestamp,
                });
                uploaded++;
            } else if (!local && cloud) {
                // Cloud only — download
                const data = await this.downloadSave(cloud.id);
                const b64 = this._uint8ToBase64(data);
                const props = cloud.appProperties || {};
                localStorage.setItem(key, JSON.stringify({
                    data: b64,
                    timestamp: props.timestamp || cloud.modifiedTime,
                    title: props.gameTitle || '',
                }));
                downloaded++;
            } else if (local && cloud) {
                // Both exist — compare timestamps
                const parsed = JSON.parse(local);
                const localTime = new Date(parsed.timestamp).getTime();
                const cloudTime = new Date(cloud.modifiedTime).getTime();

                if (localTime > cloudTime) {
                    const data = Uint8Array.from(atob(parsed.data), c => c.charCodeAt(0));
                    await this.uploadSave(cloudName, data, {
                        title: parsed.title,
                        timestamp: parsed.timestamp,
                    });
                    uploaded++;
                } else if (cloudTime > localTime) {
                    const data = await this.downloadSave(cloud.id);
                    const b64 = this._uint8ToBase64(data);
                    const props = cloud.appProperties || {};
                    localStorage.setItem(key, JSON.stringify({
                        data: b64,
                        timestamp: props.timestamp || cloud.modifiedTime,
                        title: props.gameTitle || '',
                    }));
                    downloaded++;
                }
            }
        }

        // Sync battery RAM (SRAM) files
        for (let i = 0; i < localStorage.length; i++) {
            const key = localStorage.key(i);
            if (!key.startsWith('rugb-sram-')) continue;
            const gameTitle = key.slice('rugb-sram-'.length);
            const cloudName = `sram-${gameTitle}.sav`;
            const cloud = cloudMap.get(cloudName);

            if (!cloud) {
                const b64 = localStorage.getItem(key);
                const data = Uint8Array.from(atob(b64), c => c.charCodeAt(0));
                await this.uploadSave(cloudName, data, { title: gameTitle });
                uploaded++;
            }
        }

        // Download cloud SRAM that we don't have locally
        for (const [name, file] of cloudMap) {
            if (!name.startsWith('sram-')) continue;
            const gameTitle = name.slice(5, -4); // remove "sram-" and ".sav"
            const key = `rugb-sram-${gameTitle}`;
            if (!localStorage.getItem(key)) {
                const data = await this.downloadSave(file.id);
                localStorage.setItem(key, this._uint8ToBase64(data));
                downloaded++;
            }
        }

        return { uploaded, downloaded };
    }

    _uint8ToBase64(data) {
        let binary = '';
        for (let i = 0; i < data.length; i++) binary += String.fromCharCode(data[i]);
        return btoa(binary);
    }
}
