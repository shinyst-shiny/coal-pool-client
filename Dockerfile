# Use an official Rust image as the base image for building
FROM rust:1.78 as builder

# Install the excalivator-client Rust program
RUN cargo install excalivator-client

# Use a lightweight Ubuntu image for the final container
FROM ubuntu:22.04

# Install necessary runtime dependencies
RUN apt-get update && apt-get install -y \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the installed binary from the builder stage
COPY --from=builder /usr/local/cargo/bin/excalivator-client /app/excalivator

# Set the working directory
WORKDIR /app

### HOW TO RUN IT ###
# 1) Run in a terminal: docker build -t excalivator:latest .
# 2) Run in a terminal: docker run -it -v /outer/path/to/key.json:/app/key.json excalivator:latest ./excalivator --keypair /app/key.json signup
# 3) Run in a terminal: docker run -d -v /outer/path/to/key.json:/app/key.json excalivator:latest ./excalivator --keypair /app/key.json mine --threads N --buffer 0
# /outer/path/to/key.json is your personal path to your private key, while in --threads replace N with the amount of threads you want to use from your PC