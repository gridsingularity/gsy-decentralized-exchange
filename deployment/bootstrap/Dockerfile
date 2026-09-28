# One-shot bootstrap image for the local demo: registers the trading user and
# the exchange operator on-chain via sudo. Exits 0 when done.
FROM node:20-slim

WORKDIR /app

# Install deps first for layer caching. @polkadot/api is pure JS + prebuilt wasm,
# so no native build toolchain is required.
COPY package.json ./
RUN npm install --omit=dev --no-audit --no-fund

COPY register.js ./

CMD ["node", "register.js"]
