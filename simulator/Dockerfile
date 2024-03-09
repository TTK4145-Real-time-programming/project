# amd64 Ubuntu is used as a parent image
FROM amd64/ubuntu:latest

# Working directory
WORKDIR /simulator

# Copy the current directory contents into the container
COPY ./simulator_linux /simulator/
COPY ./simulator.con /simulator/

# Command to run the application
CMD ["./simulator_linux"]