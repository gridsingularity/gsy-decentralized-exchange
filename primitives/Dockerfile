# gsy-primitives-integration-test/Dockerfile
# (or place at ./primitives/Dockerfile — adjust build context accordingly)

FROM lukemathwalker/cargo-chef:latest-rust-1 AS chef
WORKDIR /var/www/gsy-decentralized-exchange

# ---- Planner: compute the dependency recipe from the full workspace ----
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# ---- Builder: cache deps, then build the primitives test artifacts ----
FROM chef AS builder
COPY --from=planner /var/www/gsy-decentralized-exchange/recipe.json recipe.json
# Build & cache all workspace dependencies (not the app code yet)
RUN cargo chef cook --tests --recipe-path recipe.json
COPY . .
# Compile the primitives integration tests without running them,
# so the container is ready to `cargo test` fast at run time.
RUN cargo test --no-run -p primitives --tests

# The image stays at the builder stage; the compose service supplies the command.