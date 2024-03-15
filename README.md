This is unfinished!
===============

To run project, use command:
RUST_LOG=trace,network_rust=off cargo run

Ensure that the config has these settings:
Same message and peer port on all elevators
n_floors = 4
driver_address = "localhost"
driver_port = 15657

1. Install and run Docker: [Docker](https://www.docker.com)


2. Create docker image using this command:
```
docker build -t <image-name> .
```

3. Create a network:
```
docker network create <network-name>
```

4. (Alternative 1. Not recommended) Create two or more instances of the docker image to simulate multiple devices on the same network (open two terminals):
```
docker run --name <instance1-name> --network <network-name> <image-name>
```
```
docker run --name <instance2-name> --network <network-name> <image-name>
```

4. (Alternative 2. Recommended) Open up a Dev Containter in VSCode using the docker image, and open up two terminals.

5. Create two instances of simulators by running the simulator scripts. Note that the two instances must run on two different ports,
which can be set as an input parameter to the executable e.g:
```
./simulator/simulator_macos --port 15657
```
These ports must also be specified in the `config.toml` file when running the script on a specific simulator.