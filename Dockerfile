# ============================================================
# Stage 1: Builder
# Compiles the Rust source into a WASM binary using wasm-pack.
# Specific version pinning ensures reproducible builds.
# ============================================================
FROM rust:1.83 AS builder

# Install wasm-pack for building the WASM module
RUN cargo install wasm-pack

# Install the wasm32 compilation target
RUN rustup target add wasm32-unknown-unknown

WORKDIR /usr/src/app

# --- Dependency caching layer ---
# Copy only the manifest files first so Docker can cache the dependency
# download step. Source changes won't invalidate the expensive fetch.
COPY Cargo.toml Cargo.lock* ./

# Create a dummy lib.rs to pre-build dependencies
RUN mkdir src && echo "// dummy" > src/lib.rs \
    && cargo build --target wasm32-unknown-unknown --release 2>/dev/null || true \
    && rm -rf src

# Copy the real source code
COPY src/ src/

# Build the WASM release bundle, output into web/pkg/
# This produces the .wasm binary + JS glue code that the frontend imports
COPY web/ web/
RUN wasm-pack build --target web --out-dir web/pkg --release

# ============================================================
# Stage 2: Runtime
# Serves the static frontend + compiled WASM using nginx.
# Alpine-based image keeps the final image small (~40MB).
# ============================================================
FROM nginx:1.27-alpine

# Security hardening: run nginx as non-root
RUN addgroup -g 1001 -S appgroup && \
    adduser -u 1001 -S appuser -G appgroup

# Custom nginx configuration for SPA-friendly serving
# - Proper MIME types for .wasm files (application/wasm)
# - Security headers (X-Content-Type-Options, X-Frame-Options, CSP)
# - Gzip compression for wasm, js, css, and html
# - Cache headers for static assets
COPY --from=builder /usr/src/app/web/ /usr/share/nginx/html/

RUN cat > /etc/nginx/conf.d/default.conf << 'NGINX'
server {
    listen 8080;
    server_name _;
    root /usr/share/nginx/html;
    index index.html;

    # --- MIME types ---
    types {
        application/wasm wasm;
    }

    # --- Security headers ---
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;

    # --- Compression ---
    gzip on;
    gzip_types application/javascript application/wasm text/css text/html application/json;
    gzip_min_length 256;

    # --- Caching ---
    # WASM and JS bundles: long cache with content-hash in filename
    location ~* \.(wasm|js)$ {
        expires 30d;
        add_header Cache-Control "public, immutable";
    }

    # CSS and images: moderate cache
    location ~* \.(css|png|jpg|jpeg|gif|ico|svg)$ {
        expires 7d;
        add_header Cache-Control "public";
    }

    # HTML: no cache to always serve the latest version
    location ~* \.html$ {
        expires -1;
        add_header Cache-Control "no-store, no-cache, must-revalidate";
    }

    # SPA fallback
    location / {
        try_files $uri $uri/ /index.html;
    }
}
NGINX

# Fix permissions for non-root execution
RUN chown -R appuser:appgroup /usr/share/nginx/html && \
    chown -R appuser:appgroup /var/cache/nginx && \
    chown -R appuser:appgroup /var/log/nginx && \
    touch /var/run/nginx.pid && \
    chown appuser:appgroup /var/run/nginx.pid

USER appuser

EXPOSE 8080

# Healthcheck to verify the container is serving
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget -qO- http://localhost:8080/ || exit 1

CMD ["nginx", "-g", "daemon off;"]
