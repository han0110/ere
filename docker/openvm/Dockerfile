ARG BASE_IMAGE_TAG=latest
FROM ere-base:${BASE_IMAGE_TAG}

# The ere-base image provides Rust, Cargo, and common tools.
# We operate as root for SDK installation.

# Copy the OpenVM SDK installer script from the workspace context
COPY scripts/sdk_installers/install_openvm_sdk.sh /tmp/install_openvm_sdk.sh
RUN chmod +x /tmp/install_openvm_sdk.sh

# Run the OpenVM SDK installation script.
# This script installs a specific toolchain
# and installs cargo-openvm.
RUN /tmp/install_openvm_sdk.sh && rm /tmp/install_openvm_sdk.sh

# The specific Rust toolchain for OpenVM is now installed.
# cargo-openvm is installed in $CARGO_HOME/bin, which is already in PATH from ere-base.
ENV OPENVM_TOOLCHAIN_VERSION="nightly-2025-02-14"

# Verify cargo-openvm is accessible with the correct toolchain
RUN cargo "+${OPENVM_TOOLCHAIN_VERSION}" openvm --version

# Copy the entire ere project context
# The WORKDIR is /app from the base image
WORKDIR /app
COPY . .

# Run tests
RUN echo "Running tests for ere-openvm library..." && \
    cargo test --release -p ere-openvm --lib -- --color always

CMD ["/bin/bash"] 