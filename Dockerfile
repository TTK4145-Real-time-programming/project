# Use an amd64 Ubuntu image as a parent image
FROM amd64/ubuntu:latest

# Install dependencies needed for Rust and your application
RUN apt-get update && apt-get install -y curl build-essential

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Add Rust to the PATH
ENV PATH="/root/.cargo/bin:${PATH}"

# Set the working directory in the container
WORKDIR /usr/src/project

# Copy the current directory contents into the container at /usr/src/project
COPY . .

# Compile application
RUN cargo build --release
RUN chmod +x ./src/coordinator/hall_request_assigner

# Specify the command to run on container start
CMD ["./target/release/project"]
