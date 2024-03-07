# amd64 Ubuntu is used as a parent image
FROM amd64/ubuntu:latest

# Dependencies needed for Rust
RUN apt-get update && apt-get install -y curl build-essential

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Add Rust to the PATH
ENV PATH="/root/.cargo/bin:${PATH}"

# Set the working
WORKDIR /usr/src/project

# Copy the current directory contents into the container
COPY . .

# Compile application
RUN cargo build --release
RUN chmod +x ./src/coordinator/hall_request_assigner

# Command to run the application
CMD ["./target/release/project"]
