FROM rust:1.49 as build

# create a new empty shell project
RUN USER=root cargo new --bin matching-engine
WORKDIR /gsy-matching-engine

# copy manifests
COPY ./Cargo.toml ./Cargo.toml

# cache dependencies
RUN cargo build --release
RUN rm src/*.rs

# copy source tree
COPY ./src ./src

# build for release
RUN rm ./target/release/deps/gsy-matching-engine*
RUN cargo build --release

# final base
FROM rust:1.49

# copy the build artifact from the build stage
COPY --from=build /matching-engine/target/release/gsy-matching-engine .

# set the startup command to run your binary
CMD ["./gsy-matching-engine"]