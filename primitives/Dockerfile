FROM rust:1.95-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /var/www/primitives

# ---- Planner: compute the dependency recipe from the primitives crate only ----
FROM chef AS planner
COPY ./primitives /var/www/primitives
RUN cargo chef prepare --recipe-path recipe.json

# ---- Builder: cache deps, then build the primitives test artifacts ----
FROM chef AS builder
COPY --from=planner /var/www/primitives/recipe.json recipe.json
# Build & cache the primitives dependencies (not the crate code yet)
RUN cargo chef cook --tests --recipe-path recipe.json
COPY ./primitives /var/www/primitives
# Compile the primitives tests without running them,
# so the container is ready to `cargo test` fast at run time.
RUN cargo test --no-run

# The image stays at the builder stage; the compose service supplies the command.
