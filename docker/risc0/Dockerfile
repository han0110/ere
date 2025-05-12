ARG BASE_IMAGE_TAG=latest
FROM ere-base:${BASE_IMAGE_TAG}

ARG USERNAME=ere_user
USER root

# Ensure Cargo/Rustup environment variables are set from the base image for SDK script
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

# Copy and run the Risc0 SDK installer script
COPY scripts/sdk_installers/install_risc0_sdk.sh /tmp/install_risc0_sdk.sh
RUN chmod +x /tmp/install_risc0_sdk.sh

# Run the script without version arguments to install latest
# TODO: We need to change this in all scripts so that we can fix the version in CI
RUN /tmp/install_risc0_sdk.sh

# Verify Risc0 installation (script also does this, but good for Dockerfile sanity)
RUN echo "Verifying Risc0 installation in Dockerfile (post-script)..." && cargo risczero --version

# --- Run tests ---
RUN echo "Running tests for ere-risczero library..." && \
    cargo test --release -p ere-risczero --lib -- --color always


CMD ["/bin/bash"] 