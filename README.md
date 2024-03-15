# Distributed Elevator System

## Introduction
This project implements a distributed system utilizing a peer-to-peer network architecture, designed for the "Real-time programming" course at the Norwegian University of Science and Technology (NTNU). It facilitates the operation of multiple elevator instances within an elevator lab environment, ensuring a seamless and efficient elevator control system.

## Getting Started

### Prerequisites
- Ensure that Rust is installed on your system. If Rust is not installed, follow the installation instructions on the [official Rust website](https://www.rust-lang.org/tools/install).
- Amd64 architecture to run linux executables

### Running the Project
To launch an elevator instance, open a terminal and execute the following command:

```bash
cargo run
```

For increased verbosity and detailed logging, use:

```bash
RUST_LOG=trace,network_rust=off cargo run
```

The same command can be initiated on multiple computers to initiate multiple elevators working in tandem within the peer-to-peer network.

### Configuration
For the distributed system to function correctly, it is essential that all elevator instances share the same network configuration. Place the following settings within the `config.toml` file:

```rust
[network]
msg_port = 19735
peer_port = 19738
```

The elevator server can be initiated by running the following command at one of the computers in the real-time lab:

```bash
elevatorserver
```

The hardware module must know which port to use for hardware API calls, and this defaults to 15657. Place this in the `config.toml` file:

```rust
[hardware]
n_floors = 4
driver_address = "localhost"
driver_port = 15657
```

Use `n_floors` = 4 at the real-time lab.