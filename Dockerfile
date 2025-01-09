# Use a lightweight Ubuntu image as the base
FROM ubuntu:22.04

# Install necessary runtime dependencies and tools for downloading
RUN apt-get update && apt-get install -y \
    libssl-dev \
    ca-certificates \
    curl \
    jq \
    && rm -rf /var/lib/apt/lists/*

# Set the working directory
WORKDIR /app

# Fetch the latest release version and download the binary
RUN LATEST_VERSION=$(curl -s https://api.github.com/repos/shinyst-shiny/coal-pool-client/releases/latest | jq -r .tag_name) && \
    curl -L "https://github.com/shinyst-shiny/coal-pool-client/releases/download/${LATEST_VERSION}/excalivator-client-linux-aarch64.tar.gz" | tar xz && \
    chmod +x excalivator-client

### HOW TO RUN IT ###
# 1) Run in a terminal: docker build -t excalivator:latest .
# 2) Run in a terminal: docker run -d excalivator:latest ./excalivator-client mine-public-key --pubkey PUB_KEY --threads N --buffer 0