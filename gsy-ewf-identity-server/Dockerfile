# Stage 1: Build
FROM node:18-alpine AS build

WORKDIR /app

# Copy package.json file
COPY package.json ./

# Install dependencies (using install instead of ci)
RUN npm install

# Copy source files and build
COPY . .
RUN npm run build

# Stage 2: Production
FROM node:18-alpine AS production

WORKDIR /app

# Create a non-root user
RUN addgroup -S appgroup && adduser -S appuser -G appgroup
USER appuser

# Copy build artifacts and node_modules
COPY --from=build --chown=appuser:appgroup /app/dist ./dist
COPY --from=build --chown=appuser:appgroup /app/node_modules ./node_modules
COPY --from=build --chown=appuser:appgroup /app/package*.json ./

# Expose the API port
EXPOSE 3000

# Run the application.
# NOTE the dist/src path: tsconfig.json declares no "include"/"exclude", so tsc's
# rootDir is inferred as the common ancestor of src/ and test/ (i.e. the project
# root) and the entrypoint lands at dist/src/main.js, not dist/main.js. The
# original `node dist/main` never resolved. The tidier fix is a tsconfig.build.json
# that excludes test/ (Nest's own default), but that changes what `npm test`
# compiles, so it belongs in its own change rather than in deployment wiring.
CMD ["node", "dist/src/main"]