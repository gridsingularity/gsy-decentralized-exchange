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

# Run the application
CMD ["node", "dist/main"]