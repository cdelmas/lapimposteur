#!/bin/bash

docker build --network=host --no-cache=true -t rabbitmq-with-ssl ..
