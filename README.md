# Lapimposteur

A RabbitMQ Imposter, able to stub amqp consumers by reacting to messages, and publishers by generating messages.

:warning: this project is under active development, and currently unstable.

This program is distributed under the terms of the GNU GPL-3.0 or later license.

## Purpose

Lapimposteur is a generic "over-the-wire" amqp stub, that can stub any microservice, allowing full-local development.

## Usage

You need to build Lapimposteur to run it. You can use either cargo or Docker for this purpose.

### Build and run

#### Using Docker

Install [Docker](https://docs.docker.com/install/).

Then, build an image using

```
docker build -t lapimposteur:0 .
```

This can take time on the first build.

Then, running Lapimposteur is as easy as a command line:

```
docker run -it --name lapimposteur -e RUST_LOG=lapimposteur_lib=trace -v $(pwd)/examples:/conf lapimposteur:0 -c /conf/config.json
```

Some explanations about this command line:

- you need to mount a volume to give access to the config file
- you can configure the logging levels using the [`RUST_LOG`](https://docs.rs/env_logger/*/env_logger/) environment variable

#### Using cargo

Install [rustup](https://rustup.rs/).

Build using cargo:

```
cargo build
```

Then, finally run it:

```
RUST_LOG=lapimposteur_lib=trace cargo run -- -c /conf/config.json
```

### Configuration

Configuration consists in a json file.

:warning: Currently, there is no schema, as it is early stage, but when it becomes stable, one will be provided.

To understand the file, a presentation of the model is necessary. A single running Lapimposter start an imposter,
which is made of _reactors_ and _generators_.

A _reactor_ is a consumer, which will bind a queue to a given exchange/routing key, and will react with _actions_ when
consuming a message. These actions are basically sending messages (responses or events), with an optional delay.

A generator (coming soon) simply publish messages based on a cron schedule.

The generale structure of the file is as follow:

```
{
  "connection": "xxx",
  "reactors": [
    ...
  ],
  "generators": [

  ]
}

```

Please note that generators are not handled yet.

#### Configuring the connection to RabbitMQ

Lapimposteur uses a connection to consume messages, and another for publishing messages.

```
{
  "connection": "amqps://bob:bob@myrabbit.com:5671/virthost",
  "reactors": [
    ...
  ]
}

```

Lapimposteur supports TLS connections.

#### Configuring a reactor

Reactors are the core of Lapimposteur. As previously stated, each reactor creates a channel, binds a queue to an exchange/routing key, and expects that received messages on this queue have the same semantic.

Each reactor configuration have this general structure:

```
{
  "queue": "bob-q-1",
  "routing_key": "r.k.1",
  "exchange": "bob-x",
  "action": [
    {
      "to": { "exchange": "bob-x", "routingKey": "r.k.2" },
      "variables": {
        "uuid": { "type": "UuidGen" },
        "message_id": { "type": "UuidGen" }
      },
      "payload": { "File": "/cnf/payload.tpl" },
      "headers": {
        "content_type": { "Lit": "application/json" },
        "message_id": { "VarRef": { "Str": "message_id" } }
      },
      "schedule": { "seconds": 0 }
    }
  ]
}
```

̀`queue`, `routing_key` and `exchange` are the consumer definition. The interesting part here is the `action` specification, which is an array of interactions.

##### Destination (`to`)

Each action will send a message, so you need to tell where to send it:

```
{
  "to": { "exchange": "bob-x", "routingKey": "r.k.2" },
  "variables": {
    ...
  },
  "payload": { ... },
  "headers": {
    ...
  },
  "schedule": { ... }
}
```

This configuration will make the reactor send the response to `bob-x` with routing key `r.k.2`. If `routingKey` is not set, the reactor will try to get the `reply_to` header from the incoming message. If `exchange` is not set, the default exchange `""` will be used.

##### Schedule

Next is the `schedule`: easy, give the delay in seconds, in which the message is sent:

```
{
  "to": { ... },
  "variables": {
    ...
  },
  "payload": { ... },
  "headers": {
    ...
  },
  "schedule": { seconds: 3 }
}
```

To send a message immediately, set `seconds` to `0`.

##### Variables

This part is crucial: Lapimposteur can generate random values or extract them from the incoming message, in order to customize the message it sends. All these values are stored into _variables_:

```
{
  "to": { ... },
  "variables": {
    "uuid": { "type": "UuidGen" },
    "message_id": { "type": "UuidGen" }
  },
  "payload": { ... },
  "headers": {
    ...
  },
  "schedule": { ... }
}
```

`variables` is a map (a json object) with a name, e.g. `uuid`, and a value specification. In the given example, the reactor will generate a random uuid v4 for the variables `named uuid` and `message_id`. These variables will then be used in _headers_ and _payload_.

Here is the exhaustive list of variables specifications:

- `{ "type": "UuidGen" }` generates a random uuid v4
- `{ "type": "StrGen", "param": 5 }` generates a random alphanumeric string of size 5
- `{ "type": "IntGen" }` generates a random integer
- `{ "type": "RealGen" }` generates a random real number
- `{ "type": "DateTime" }` gives the current time, formatted as an ISO string
- `{ "type": "Timestamp" }` gives the current time as a number
- `{ "type": "Lit", "param": { "Str": "value" } }` gives the literal string `"value"`
- `{ "type": "Lit", "param": { "Int": 42 } }` gives the literal int `42`
- `{ "type": "Lit", "param": { "Real": 1.2 } }` gives the literal real number `1.2`
- `{ "type": "IntHeader", "param": "headerName" }` gives the int value of the incoming message's header `headerName`
- `{ "type": "StrHeader", "param": "headerName" }` gives the string value of the incoming message's header `headerName`
- `{ "type": "Env", "param": "HOST" }` gives the string value of the environment variable `HOST`
- `{ "type": "StrJsonPath", "param": "$.value" }` gives the string value extracted from the incoming message's body using the json path `$.value`
- `{ "type": "IntJsonPath", "param": "$.value" }` gives the int value extracted from the incoming message's body using the json path `$.value`

##### Headers

`headers` will be reported as-is in the sent messages:

```
{
  "to": { ... },
  "variables": {
    ...
  },
  "payload": { ... },
  "headers": {
    "content_type": { "Lit": "application/json" },
    "message_id": { "VarRef": { "Str": "message_id" } }
  },
  "schedule": { ... }
}
```

You can either set a literal (hardcoded) value, using `{ "Lit": "theValue" }` or `{ "Lit": 42 }`, or a reference to a variable, using `{ "VarRef": { "Str": "message_id" } }` or `{ "VarRef": { "Int": 69 } }`.

:warning: the types of variable references must match the declared type of the variable, or the reactor will fail.

##### Payload

Finally, you can configure the payload of the message:

```
{
  "to": { ... },
  "variables": {
    ...
  },
  "payload": { "File": "/cnf/payload.tpl" },
  "headers": {
    ...
  },
  "schedule": { ... }
}
```

The `File` is an absolute path to a text file, which contains the payload template. Such a template can contain variables:

```
{"message": "my id is: {{ uuid }}"}
```

Here, the reactor will inject the value of the variable `uuid` before sending the message.

Please note that the template is considered as a string, so it is not parsed (because Lapimposteur is agnostic of the format, although it only support text messages at this time).

It is possible, for example in the case of very short payload, to configure it directly in the configuration file:

```
{
  "to": { ... },
  "variables": {
    ...
  },
  "payload": { "Inline": "{ \"msg\": \"This is the message, id {{ uuid }}\" }" },
  "headers": {
    ...
  },
  "schedule": { ... }
}
```

In this case, be careful to escape the quotes, as the payload is a string embedded in a json file, and this file is parsed!

##### Examples

See `examples/config.json` for a working example.

To run the example:

- start RabbitMQ: `examples/run-rabbit.sh`
- then setup RabbitMQ: `examples/rabbit_setup.sh`
- setup a client: `examples/rabbit_client.sh`
- run Lapimposteur: `examples/run.sh`
- send messages from the client: `examples/client_send.sh` multiple times (see the logs in the Lapimposteur console)
- get responses back: `examples/client_get.sh` (you may need to execute it more than once, as it only get the last 10 messages, then ack them)
- once you are done, stop Lapimposteur `examples/stop.sh`, then RabbitMQ `examples/stop-rabbit.sh`

## Troubleshooting

Logs should give you enough information about problems. If not, please file an issue.

## Contributing

### How to request a feature?

If you feel Lapimposteur need an improvement, please open an issue.

### How to file a bug?

If you feel Lapimposteur has some king of bug, please open an issue.

### How to add a feature or fix a bug?

Just fork Lapimposteur, and open a pull request.

### How to build?

Lapimposteur is a [Rust](https://www.rust-lang.org/) program, so use `cargo` to build it.

### Architecture

bin -> the command line binary. Run the server provided by the lib.
lib -> the server, along with the config structures.
