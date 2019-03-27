# Lapimposteur

A RabbitMQ Imposter, able to stub amqp consumers by reacting to messages, and publishers by generating messages.

:warning: this project is under active development, and currently unstable.

## Purpose

TODO

## Usage

### Command line

```
docker run -it --network=host --name lapimposteur -e RUST_LOG=lapimposteur_lib=trace -v $(pwd)/examples:/conf lapimposteur:0 -c /conf/config.json
```

### Configuration

TODO: exhaustive configuration.

See `examples` for a working example.

## Troubleshooting

Logs should give you enough information about problems. If not, please file an issue.

## Contributing

TODO: extract to a new file

### How to request a feature?

### How to file a bug?

### How to add a feature or fix a bug?

### How to build?

### Structure

bin -> the command line binary. Run the server provided by the lib.
lib -> the server, along with the config structures.
