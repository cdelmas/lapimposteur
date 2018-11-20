# Lapimposteur

A RabbitMQ Imposter, able to stub amqp consumers by reacting to messages, and publishers by generating messages.

:warning: this project is under development, and not usable yet. Stay tuned!

## Structure

bin -> the server. Runs an amqp service, (with a control channel [experimental]). Configured by file, or dynamically using ctl [experimental].
lib -> defines the commands that the server accepts. Used by both bin and ctl.
[experimental] ctl -> server control cli. Sends commands to the server.

## Usage

### Command line

### Configuration

## Troubleshooting

## Contributing TODO: extract to a new file

### How to build?

### How to request for a feature?

### How to file a bug?

### How to add a feature or fix a bug?
