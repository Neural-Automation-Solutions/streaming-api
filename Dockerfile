# Use the official Rust image from the Docker Hub
FROM rust:latest

# Create a new directory for the app
WORKDIR /usr/src/myapp

# Copy the Cargo.toml and Cargo.lock (if available) to the working directory
COPY Cargo.toml Cargo.lock ./

# Copy the source code to the container
COPY src ./src
COPY .env ./.env

# Build the application
RUN cargo build --release

# Run the application by default
CMD ["./target/release/stream-api"]