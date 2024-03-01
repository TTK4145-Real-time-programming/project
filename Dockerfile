# Use the official Rust image as a parent image
FROM rust:latest

# Set the working directory in the container
WORKDIR /usr/src/project

# Copy the current directory contents into the container at /usr/src/myapp
COPY . .

# Compile application
RUN cargo build --release

# Specify the command to run on container start
CMD ["./target/release/project"]
